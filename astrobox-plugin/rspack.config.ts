import { defineConfig } from "@rspack/cli";
import { rspack } from "@rspack/core";

const targets = ["chrome 65"];

export default defineConfig({
  mode: "production",
  devtool: false,
  entry: {
    main: "./src/index.js"
  },
  resolve: {
    extensions: [".ts", ".js"]
  },
  output: {
    filename: "[name].js",
    clean: true
  },
  module: {
    rules: [
      {
        test: /\.js$/,
        use: [
          {
            loader: "builtin:swc-loader",
            options: {
              jsc: {
                parser: { syntax: "ecmascript" }
              },
              env: { targets }
            }
          }
        ]
      }
    ]
  },
  plugins: [
    new rspack.CopyRspackPlugin({
      patterns: [
        { from: "manifest.json", to: "." },
        { from: "icon.png", to: "." }
      ]
    })
  ]
});

