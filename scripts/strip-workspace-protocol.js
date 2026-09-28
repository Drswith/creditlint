"use strict";

const fs = require("node:fs");

const DEPENDENCY_KEYS = [
  "dependencies",
  "optionalDependencies",
  "peerDependencies",
  "devDependencies",
];

function stripWorkspaceProtocol(pkg) {
  const result = JSON.parse(JSON.stringify(pkg));

  for (const key of DEPENDENCY_KEYS) {
    const deps = result[key];

    if (deps && typeof deps === "object") {
      for (const [name, spec] of Object.entries(deps)) {
        if (typeof spec === "string" && spec.startsWith("workspace:")) {
          deps[name] = spec.slice("workspace:".length);
        }
      }
    }
  }

  return result;
}

function stripWorkspaceProtocolFile(filePath) {
  const pkg = JSON.parse(fs.readFileSync(filePath, "utf8"));
  const stripped = stripWorkspaceProtocol(pkg);
  fs.writeFileSync(filePath, JSON.stringify(stripped, null, 2) + "\n");
}

if (require.main === module) {
  const filePath = process.argv[2];

  if (!filePath) {
    console.error("usage: strip-workspace-protocol.js <package.json>");
    process.exit(2);
  }

  stripWorkspaceProtocolFile(filePath);
}

module.exports = { stripWorkspaceProtocol, stripWorkspaceProtocolFile };