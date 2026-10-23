export async function load(order) {
  const name = `dual-${order}-first-regression`;
  let imported;
  let required;
  if (order === "import") {
    imported = await import(name);
    required = require(name);
  } else {
    required = require(name);
    imported = await import(name);
  }
  return {
    imported,
    required,
    importedAgain: await import(name),
    requiredAgain: require(name),
  };
}
