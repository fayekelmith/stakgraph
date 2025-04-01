import { z } from "zod";
import { Tool } from "./index.js";
import { parseSchema } from "./utils.js";
import { relevant_node_types, NodeType } from "../graph/types.js";
import * as G from "../graph/graph.js";

export const GetNodesSchema = z.object({
  node_type: z
    .enum(relevant_node_types() as [string, ...string[]])
    .optional()
    .describe("Type of node to retrieve (e.g. 'function', 'class', etc)."),
  concise: z
    .boolean()
    .optional()
    .describe(
      "Whether to return a concise response (only the name and filename)."
    ),
});

export const GetNodesTool: Tool = {
  name: "get_nodes",
  description: "Retrieve all nodes of a specific type from the codebase.",
  inputSchema: parseSchema(GetNodesSchema),
};

export async function getNodes(args: z.infer<typeof GetNodesSchema>) {
  console.log("=> Running get_nodes tool with args:", args);
  const result = await G.get_nodes(
    args.node_type as NodeType,
    args.concise ?? false
  );
  return {
    content: [
      {
        type: "text",
        text: JSON.stringify(result),
      },
    ],
  };
}
