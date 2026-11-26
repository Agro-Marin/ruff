import { fetchPlayground, savePlayground } from "./api";
import {
  generatePlaygroundMarkdown,
  generatePlaygroundMarkdownLink,
} from "./markdown";

interface Workspace {
  files: { [name: string]: string };

  current: string;
}

async function shareUrl(workspace: Workspace): Promise<string> {
  const id = await savePlayground(workspace);
  return `${window.location.origin}/${encodeURIComponent(id)}`;
}

export async function persist(workspace: Workspace): Promise<void> {
  const url = await shareUrl(workspace);
  await navigator.clipboard.writeText(url);
}

export async function copyAsMarkdownLink(workspace: Workspace): Promise<void> {
  const url = await shareUrl(workspace);
  await navigator.clipboard.writeText(generatePlaygroundMarkdownLink(url));
}

export async function copyAsMarkdown(workspace: Workspace): Promise<void> {
  const url = await shareUrl(workspace);
  await navigator.clipboard.writeText(
    generatePlaygroundMarkdown(workspace.files, url),
  );
}

export async function restore(): Promise<Workspace | null> {
  const id = window.location.pathname.slice(1);
  if (id !== "") {
    const playground = await fetchPlayground(id);
    if (playground == null) {
      return null;
    }

    return playground;
  }

  return restoreLocal();
}

export function persistLocal(workspace: Workspace) {
  let totalLength = 0;
  for (const fileContent of Object.values(workspace.files)) {
    totalLength += fileContent.length;

    if (totalLength > 500_000) {
      return;
    }
  }

  localStorage.setItem("workspace", JSON.stringify(workspace));
}

function restoreLocal(): Workspace | null {
  const workspace = localStorage.getItem("workspace");

  if (workspace == null) {
    return null;
  } else {
    return JSON.parse(workspace);
  }
}
