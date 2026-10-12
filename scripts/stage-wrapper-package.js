#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

function rewriteOptionalDependencies(manifest) {
  if (!manifest.optionalDependencies) {
    return manifest;
  }

  const version = manifest.version;
  for (const name of Object.keys(manifest.optionalDependencies)) {
    manifest.optionalDependencies[name] = version;
  }

  return manifest;
}

function stageWrapperPackage(sourceDir, targetDir) {
  const manifest = JSON.parse(
    fs.readFileSync(path.join(sourceDir, "package.json"), "utf8"),
  );
  rewriteOptionalDependencies(manifest);

  fs.mkdirSync(path.join(targetDir, "bin"), { recursive: true });
  fs.writeFileSync(
    path.join(targetDir, "package.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );

  const binSource = path.join(sourceDir, "bin", "creditlint.js");
  const binTarget = path.join(targetDir, "bin", "creditlint.js");
  fs.copyFileSync(binSource, binTarget);
  fs.chmodSync(binTarget, 0o755);

  const readme = path.join(sourceDir, "README.md");
  if (fs.existsSync(readme)) {
    fs.copyFileSync(readme, path.join(targetDir, "README.md"));
  }
}

module.exports = {
  rewriteOptionalDependencies,
  stageWrapperPackage,
};

if (require.main === module) {
  const [, , sourceDir, targetDir] = process.argv;
  if (!sourceDir || !targetDir) {
    console.error("usage: stage-wrapper-package.js <source-dir> <target-dir>");
    process.exit(2);
  }

  const manifest = JSON.parse(
    fs.readFileSync(path.join(sourceDir, "package.json"), "utf8"),
  );
  stageWrapperPackage(sourceDir, targetDir);
  console.log(`staged creditlint wrapper ${manifest.version} -> ${targetDir}`);
}