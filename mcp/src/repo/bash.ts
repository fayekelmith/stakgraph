import { spawn } from "child_process";
import * as fs from "fs";
import * as path from "path";

// Execute ripgrep commands with proper streaming
function execCommand(
  command: string,
  cwd: string,
  timeoutMs: number = 10000
): Promise<string> {
  return new Promise((resolve, reject) => {
    // Parse the ripgrep command and add explicit directory
    const parts = command.split(" ");
    const rgIndex = parts.findIndex(
      (part) => part === "rg" || part.endsWith("/rg")
    );

    // Build ripgrep arguments properly, removing quotes and adding explicit directory
    const args = parts.slice(rgIndex + 1).map((arg) => {
      // Remove surrounding quotes (both single and double)
      if (
        (arg.startsWith('"') && arg.endsWith('"')) ||
        (arg.startsWith("'") && arg.endsWith("'"))
      ) {
        return arg.slice(1, -1);
      }
      return arg;
    });
    args.push("./"); // Add explicit directory to prevent stdin detection issues

    const process = spawn("rg", args, {
      cwd,
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";
    let resolved = false;

    // Set up timeout
    const timeout = setTimeout(() => {
      if (!resolved) {
        process.kill("SIGKILL");
        resolved = true;
        reject(new Error(`Command timed out after ${timeoutMs}ms`));
      }
    }, timeoutMs);

    process.stdout.on("data", (data) => {
      stdout += data.toString();

      // Safety check: if output gets too large, kill process and resolve
      if (stdout.length > 10000) {
        process.kill("SIGKILL");
        if (!resolved) {
          resolved = true;
          clearTimeout(timeout);
          const truncated =
            stdout.substring(0, 10000) +
            "\n\n[... output truncated due to size limit ...]";
          resolve(truncated);
        }
        return;
      }
    });

    process.stderr.on("data", (data) => {
      stderr += data.toString();
    });

    process.on("close", (code) => {
      if (!resolved) {
        resolved = true;
        clearTimeout(timeout);

        if (code === 0) {
          if (stdout.length > 10000) {
            const truncated =
              stdout.substring(0, 10000) +
              "\n\n[... output truncated to 10,000 characters ...]";
            resolve(truncated);
          } else {
            resolve(stdout);
          }
        } else if (code === 1) {
          // ripgrep returns exit code 1 when no matches found
          resolve("No matches found");
        } else {
          reject(new Error(`Command failed with code ${code}: ${stderr}`));
        }
      }
    });

    process.on("error", (error) => {
      if (!resolved) {
        resolved = true;
        clearTimeout(timeout);
        reject(error);
      }
    });
  });
}

// Get repository map using ripgrep --files
export async function getRepoMap(repoPath: string): Promise<string> {
  if (!repoPath) {
    return "No repository path provided";
  }

  if (!fs.existsSync(repoPath)) {
    return "Repository not cloned yet";
  }

  // "rg --files",
  try {
    const result = await execCommand(
      "git ls-tree -r --name-only HEAD | tree -L 3 --fromfile",
      repoPath
    );
    return result;
  } catch (error: any) {
    return `Error getting repo map: ${error.message}`;
  }
}

// Get file summary by reading first 40 lines
export function getFileSummary(
  filePath: string,
  repoPath: string,
  linesLimit: number
): string {
  if (!repoPath) {
    return "No repository path provided";
  }

  const fullPath = path.join(repoPath, filePath);

  if (!fs.existsSync(fullPath)) {
    return "File not found";
  }

  try {
    const content = fs.readFileSync(fullPath, "utf-8");
    const lines = content
      .split("\n")
      .slice(0, linesLimit || 40)
      .map((line) => {
        // Limit each line to 200 characters to handle minified files
        return line.length > 200 ? line.substring(0, 200) + "..." : line;
      });

    return lines.join("\n");
  } catch (error: any) {
    return `Error reading file: ${error.message}`;
  }
}

// Fulltext search using ripgrep
export async function fulltextSearch(
  query: string,
  repoPath: string
): Promise<string> {
  if (!repoPath) {
    return "No repository path provided";
  }

  if (!fs.existsSync(repoPath)) {
    return "Repository not cloned yet";
  }

  try {
    const result = await execCommand(
      `rg --glob '!dist' --ignore-file .gitignore -C 2 -n --max-count 10 --max-columns 200 "${query}"`,
      repoPath,
      5000
    );

    // Limit the result to 10,000 characters to prevent overwhelming output
    if (result.length > 10000) {
      return (
        result.substring(0, 10000) +
        "\n\n[... output truncated to 10,000 characters ...]"
      );
    }

    return result;
  } catch (error: any) {
    // Ripgrep returns exit code 1 when no matches found, which is not really an error
    if (error.message.includes("code 1")) {
      return `No matches found for "${query}"`;
    }
    return `Error searching: ${error.message}`;
  }
}
