export default {
  "*.{ts,tsx,css,html,mts,json}":
    "bunx biome check --fix --files-ignore-unknown=true --diagnostic-level=error --no-errors-on-unmatched",
  "*.rs": ["cargo clippy --fix --allow-dirty --allow-staged", "cargo fmt --"],
};
