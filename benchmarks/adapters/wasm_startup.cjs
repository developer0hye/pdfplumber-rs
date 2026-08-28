#!/usr/bin/env node
"use strict";

const path = require("node:path");

if (process.argv.length !== 3) {
  throw new Error("usage: wasm_startup.cjs PACKAGE_ENTRY");
}

const entry = path.resolve(process.argv[2]);
const started = process.hrtime.bigint();
require(entry);
const wallTimeNs = process.hrtime.bigint() - started;

process.stdout.write(JSON.stringify({
  clock: "monotonic-wall",
  clock_scope: "node-module-load-only",
  process_model: "fresh-process-per-sample",
  includes_process_launch: false,
  wall_time_ns: Number(wallTimeNs),
}) + "\n");
