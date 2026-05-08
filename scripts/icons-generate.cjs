/**
 * Wrapper so `npm run icons:generate` without args explains the correct syntax.
 * npm requires a space after `--` to pass the image path: `npm run icons:generate -- <path>`
 */
const { spawnSync } = require("child_process");
const path = require("path");

const imagePath = process.argv[2];
if (!imagePath) {
  console.error(
    "Missing path to your square PNG (or SVG).\n\n" +
      "Run (note the space after --):\n" +
      '  npm run icons:generate -- "C:/Users/YOU/Desktop/tapt.png"\n\n' +
      "Or:\n" +
      '  npx tauri icon "C:/Users/YOU/Desktop/tapt.png"\n',
  );
  process.exit(1);
}

const root = path.join(__dirname, "..");
const result = spawnSync("npx", ["tauri", "icon", imagePath], {
  cwd: root,
  stdio: "inherit",
  shell: true,
});
process.exit(result.status === null ? 1 : result.status);
