export async function importPackage(name) {
  return import(name);
}

export function requirePackage(name) {
  return require(name);
}
