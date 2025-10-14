import { ModelMessage, streamText, ToolSet, generateObject } from "ai";
import {
  Provider,
  getModel,
  getProviderOptions,
  ThinkingSpeed,
} from "./provider.js";

interface CallModelOptions {
  provider: Provider;
  apiKey: string;
  messages: ModelMessage[];
  tools?: ToolSet;
  parser?: (fullResponse: string) => void;
  thinkingSpeed?: ThinkingSpeed;
  cwd?: string;
  executablePath?: string;
}

export async function callModel(opts: CallModelOptions): Promise<{
  text: string;
  usage: { inputTokens: number; outputTokens: number; totalTokens: number };
}> {
  const {
    provider,
    apiKey,
    messages,
    tools,
    parser,
    thinkingSpeed,
    cwd,
    executablePath,
  } = opts;
  const model = await getModel(provider, apiKey, cwd, executablePath);
  const providerOptions = getProviderOptions(provider, thinkingSpeed);
  console.log(`Calling ${provider} with options:`, providerOptions);
  const result = streamText({
    model,
    tools,
    messages,
    temperature: 0,
    providerOptions: providerOptions as any,
  });
  let fullResponse = "";
  for await (const part of result.fullStream) {
    // console.log(part);
    switch (part.type) {
      case "error":
        throw part.error;
      case "text-delta":
        if (parser) {
          parser(fullResponse);
        }
        fullResponse += part.text;
        break;
    }
  }
  const usage = await result.usage;
  return {
    text: fullResponse,
    usage: {
      inputTokens: usage.inputTokens || 0,
      outputTokens: usage.outputTokens || 0,
      totalTokens: usage.totalTokens || 0,
    },
  };
}

interface GenerateObjectArgs {
  provider: Provider;
  apiKey: string;
  prompt: string;
  schema: any;
}

export async function callGenerateObject(args: GenerateObjectArgs): Promise<{
  object: any;
  usage: { inputTokens: number; outputTokens: number; totalTokens: number };
}> {
  const model = await getModel(args.provider, args.apiKey);
  const { object, usage } = await generateObject({
    model,
    schema: args.schema,
    prompt: args.prompt,
  });
  return {
    object,
    usage: {
      inputTokens: usage.inputTokens || 0,
      outputTokens: usage.outputTokens || 0,
      totalTokens: usage.totalTokens || 0,
    },
  };
}
