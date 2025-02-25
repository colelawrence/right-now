/** @type {import('lint-staged').Configuration} */
export default {
  "*.{ts,tsx,css,html,mts,json,js,mjs,cjs}":
    "bunx biome check --fix --files-ignore-unknown=true --diagnostic-level=error --no-errors-on-unmatched",
  "*.rs,Cargo.lock,Cargo.toml": () => ["cargo check"],
  "*.rs": (files) =>
    // only spend more time if there are a lot of changes
    files.length > 2
      ? ["cargo clippy --fix --allow-dirty --allow-staged", "cargo fmt"]
      : [`cargo fmt -- ${files.map((a) => `"${a}"`).join(" ")}`],
};
