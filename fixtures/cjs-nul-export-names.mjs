export {
  "\u0000" as leading,
  "abc\u0000def" as embedded,
  "trailing\u0000" as trailing,
  "default\u0000suffix" as defaultPrefix,
} from "./cjs-nul-export-names.cjs";
