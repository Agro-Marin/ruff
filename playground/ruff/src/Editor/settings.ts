import lzstring from "lz-string";
import { fetchPlayground, savePlayground } from "./api";

export type Settings = { [K: string]: any };

export function stringify(settings: Settings): string {
  return JSON.stringify(
    settings,
    (k, v) => {
      if (v instanceof Map) {
        return Object.fromEntries(v.entries());
      } else {
        return v;
      }
    },
    2,
  );
}

async function shareUrl(
  settingsSource: string,
  pythonSource: string,
): Promise<string> {
  const id = await savePlayground({ settingsSource, pythonSource });
  return `${window.location.origin}/${encodeURIComponent(id)}`;
}

export async function persist(
  settingsSource: string,
  pythonSource: string,
): Promise<void> {
  const url = await shareUrl(settingsSource, pythonSource);
  await navigator.clipboard.writeText(url);
}

export async function copyAsMarkdownLink(
  settingsSource: string,
  pythonSource: string,
): Promise<void> {
  const url = await shareUrl(settingsSource, pythonSource);
  await navigator.clipboard.writeText(`[Playground](${url})`);
}

export async function copyAsMarkdown(
  settingsSource: string,
  pythonSource: string,
): Promise<void> {
  const [url, toml] = await Promise.all([
    shareUrl(settingsSource, pythonSource),
    import("smol-toml"),
  ]);

  let settingsBlock: string;
  try {
    settingsBlock = `\`\`\`toml\n${toml.stringify(JSON.parse(settingsSource))}\n\`\`\``;
  } catch {
    settingsBlock = `\`\`\`json\n${settingsSource}\n\`\`\``;
  }

  await navigator.clipboard.writeText(
    `\`\`\`py
${pythonSource}
\`\`\`

${settingsBlock}

[Playground](${url})
`,
  );
}

export async function restore(): Promise<[string, string] | null> {
  const hash = window.location.hash.slice(1);
  if (hash) {
    const value = lzstring.decompressFromEncodedURIComponent(
      window.location.hash.slice(1),
    )!;
    const [settingsSource, pythonSource] = value.split("$$$");
    return [settingsSource.replaceAll("$$$$$$", "$$$"), pythonSource];
  }

  const id = window.location.pathname.slice(1);
  if (id) {
    const playground = await fetchPlayground(id);
    if (playground == null) {
      return null;
    }
    const { settingsSource, pythonSource } = playground;
    return [settingsSource, pythonSource];
  }

  return restoreLocal();
}

function restoreLocal(): [string, string] | null {
  const source = localStorage.getItem("source");

  if (source == null) {
    return null;
  } else {
    return JSON.parse(source);
  }
}

export function persistLocal({
  settingsSource,
  pythonSource,
}: {
  settingsSource: string;
  pythonSource: string;
}) {
  const totalLength = settingsSource.length + pythonSource.length;

  if (totalLength > 500_000) {
    return;
  }

  localStorage.setItem(
    "source",
    JSON.stringify([settingsSource, pythonSource]),
  );
}
