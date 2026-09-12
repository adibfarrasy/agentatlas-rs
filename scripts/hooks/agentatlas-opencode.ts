// scripts/hooks/agentatlas-opencode.ts — opencode plugin.
//
// Nudges the agent to reach for `agentatlas` before blind grep + whole-file reads. opencode has no
// per-tool-call text injection primitive, so this appends the reminder as a text part to the first
// user message of each session via the `chat.message` hook — the reminder then sits in the context
// the model sees, next to the agentatlas skill.
//
// The installer places this in ~/.config/opencode/plugins/, where opencode auto-loads it at startup.
// The reminder text is read synchronously beside the plugin to avoid the known async/sessionID race
// when pushing to output.parts.

import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"

export const AgentatlasHooks = async () => {
  let reminder = ""
  try {
    const here = fileURLToPath(new URL(".", import.meta.url))
    reminder = readFileSync(`${here}agentatlas-reminder.txt`, "utf8").trim()
  } catch {
    reminder = ""
  }

  const reminded = new Set<string>()

  return {
    "chat.message": async (input, output) => {
      if (!reminder || reminded.has(input.sessionID)) return
      reminded.add(input.sessionID)
      output.parts.push({ type: "text", text: reminder })
    },
  }
}

export default AgentatlasHooks