"use strict";

const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const wrapper = path.resolve(__dirname, "..", "bin", "creditlint.js");
const {
  candidateBinaries,
  platformPackageName,
  resolveBinary,
} = require("../bin/creditlint.js");
const {
  rewriteOptionalDependencies,
  stageWrapperPackage,
} = require("../../../scripts/stage-wrapper-package.js");

function makeFakeBinary() {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "creditlint-wrapper-"));
  const fakeBinary = path.join(tempDir, "creditlint-fake");

  fs.writeFileSync(
    fakeBinary,
    [
      "#!/usr/bin/env node",
      "const args = process.argv.slice(2);",
      "console.log(JSON.stringify(args));",
      "process.exit(args.includes('--fail') ? 7 : 0);",
      "",
    ].join("\n"),
  );
  fs.chmodSync(fakeBinary, 0o755);

  return { fakeBinary, tempDir };
}

test("CREDITLINT_BIN override receives user arguments", () => {
  const { fakeBinary, tempDir } = makeFakeBinary();

  try {
    const result = spawnSync(process.execPath, [wrapper, "check", "--stdin"], {
      env: { ...process.env, CREDITLINT_BIN: fakeBinary },
      encoding: "utf8",
    });

    assert.equal(result.status, 0);
    assert.match(result.stdout, /\["check","--stdin"\]/);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test("native exit code is forwarded", () => {
  const { fakeBinary, tempDir } = makeFakeBinary();

  try {
    const result = spawnSync(process.execPath, [wrapper, "--fail"], {
      env: { ...process.env, CREDITLINT_BIN: fakeBinary },
      encoding: "utf8",
    });

    assert.equal(result.status, 7);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test("missing native binary exits with code 2", () => {
  const missingBinary = path.join(os.tmpdir(), "creditlint-missing-native");
  const result = spawnSync(process.execPath, [wrapper, "--version"], {
    env: { ...process.env, CREDITLINT_BIN: missingBinary },
    encoding: "utf8",
  });

  assert.equal(result.status, 2);
  assert.match(result.stderr, /could not find a native creditlint binary/);
  assert.match(result.stderr, /CREDITLINT_BIN/);
});

test("CREDITLINT_BIN override is the only candidate when set", () => {
  const overridePath = path.join(os.tmpdir(), "creditlint-override");
  const candidates = candidateBinaries({
    env: { CREDITLINT_BIN: overridePath },
    platform: "darwin",
    arch: "arm64",
  });

  assert.deepEqual(candidates, [
    {
      kind: "env",
      path: overridePath,
    },
  ]);
});

test("current supported platform maps to native package name", () => {
  assert.equal(platformPackageName("darwin", "arm64"), "creditlint-darwin-arm64");
  assert.equal(platformPackageName("darwin", "x64"), "creditlint-darwin-x64");
  assert.equal(platformPackageName("linux", "arm64"), "creditlint-linux-arm64");
  assert.equal(platformPackageName("linux", "x64"), "creditlint-linux-x64");
  assert.equal(platformPackageName("win32", "x64"), "creditlint-windows-x64");
});

test("installed platform package candidate is checked before local fallbacks", () => {
  const candidates = candidateBinaries({
    env: {},
    platform: process.platform,
    arch: process.arch,
    repoRoot: "/repo",
  });

  assert.equal(candidates[0].kind, "platform-package");
  assert.match(candidates[0].packageName, /^creditlint-/);
  assert.equal(candidates.at(-2).kind, "cargo-release");
  assert.equal(candidates.at(-1).kind, "cargo-debug");
});

test("unsupported platform reports platform key", () => {
  const resolved = resolveBinary({
    env: {},
    platform: "freebsd",
    arch: "riscv64",
    repoRoot: "/definitely-missing-creditlint-repo",
  });

  assert.equal(resolved.binary, undefined);
  assert.equal(resolved.platformKey, "freebsd-riscv64");
  assert.match(resolved.error, /unsupported platform: freebsd-riscv64/);
});

test("supported platform without native binary reports missing optional package", () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "creditlint-empty-repo-"));

  try {
    const resolved = resolveBinary({
      env: {},
      platform: "linux",
      arch: "x64",
      baseDir: tempDir,
      repoRoot: tempDir,
    });

    assert.equal(resolved.binary, undefined);
    assert.equal(resolved.packageName, "creditlint-linux-x64");
    assert.match(resolved.error, /optional package creditlint-linux-x64 may not be installed/);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test("staged wrapper manifest never ships workspace: protocol references", () => {
  const sourceDir = path.resolve(__dirname, "..");
  const stageDir = fs.mkdtempSync(path.join(os.tmpdir(), "creditlint-stage-"));

  try {
    stageWrapperPackage(sourceDir, stageDir);

    const staged = JSON.parse(
      fs.readFileSync(path.join(stageDir, "package.json"), "utf8"),
    );

    assert.ok(staged.optionalDependencies, "wrapper must declare optionalDependencies");
    for (const [name, spec] of Object.entries(staged.optionalDependencies)) {
      assert.equal(
        typeof spec,
        "string",
        `${name} optionalDependency version must be a string`,
      );
      assert.ok(
        !String(spec).startsWith("workspace:"),
        `${name} must not use the workspace: protocol in published manifest (got ${spec})`,
      );
      assert.ok(
        /^\d+\.\d+\.\d+[-+0-9A-Za-z.\-]*$/.test(String(spec)),
        `${name} must be a concrete semver version in published manifest (got ${spec})`,
      );
    }

    assert.ok(
      fs.existsSync(path.join(stageDir, "bin", "creditlint.js")),
      "staged wrapper must include the bin entry",
    );
  } finally {
    fs.rmSync(stageDir, { recursive: true, force: true });
  }
});

test("rewriteOptionalDependencies rewrites workspace: refs to the manifest version", () => {
  const manifest = {
    name: "creditlint",
    version: "0.1.2",
    optionalDependencies: {
      "creditlint-darwin-arm64": "workspace:0.1.2",
      "creditlint-linux-x64": "workspace:^0.1.0",
      "creditlint-windows-x64": "0.1.2",
    },
  };

  const rewritten = rewriteOptionalDependencies(structuredClone(manifest));

  assert.deepEqual(rewritten.optionalDependencies, {
    "creditlint-darwin-arm64": "0.1.2",
    "creditlint-linux-x64": "0.1.2",
    "creditlint-windows-x64": "0.1.2",
  });
});
