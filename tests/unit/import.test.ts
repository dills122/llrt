const CWD = process.cwd();

describe("import", () => {
  it("should import a js file (absolute path)", async () => {
    const mod = await import(`${CWD}/fixtures/hello.js`);

    expect(mod.hello).toEqual("hello world!");
  });

  it("should import a json file (absolute path)", async () => {
    async function importThatAvoidsTranspilation(path: string) {
      const fn = new Function(
        "path",
        'return import(path, {with: { type: "json" }});'
      );
      return fn(path);
    }

    const mod = await importThatAvoidsTranspilation(`${CWD}/package.json`);

    expect(mod.default.private).toEqual(true);
  });

  it("should import a js file (relative path)", async () => {
    const mod = await import("../../fixtures/hello.js");

    expect(mod.hello).toEqual("hello world!");
  });

  it("should import a json file (relative path)", async () => {
    const mod = await import("../../fixtures/package.json", {
      with: { type: "json" },
    });

    expect(mod.default.private).toEqual(true);
  });

  it("should import a json file (path unspecified)", async () => {
    const mod = await import("package.json", { with: { type: "json" } });

    expect(mod.default.private).toEqual(true);
  });

  it("should have import.meta.url", async () => {
    const url = import.meta.url;
    // Verify import.meta.url structure without depending on CWD
    // (test can be run from any directory)
    expect(url.startsWith("file://")).toEqual(true);
    // Normalize path separators for cross-platform compatibility
    const normalizedUrl = url.replaceAll("\\", "/");
    expect(
      normalizedUrl.endsWith("/bundle/js/__tests__/unit/import.test.js")
    ).toEqual(true);
  });

  it("should use module for import and main for require in a dual package", async () => {
    // Keep the bare package import in an unbundled fixture so LLRT resolves it.
    const filename = `${CWD}/fixtures/dual-format-import.mjs`;
    const mod = await import(filename);

    expect(Object.keys(mod.imported)).toEqual(["format"]);
    expect(mod.imported.format).toBe("esm entry");
    expect(mod.required.format).toBe("cjs entry");
  });

  for (const order of ["import", "require"]) {
    it(`should keep dual-package entry identities and execute each once with ${order} first`, async () => {
      const { load } = await import(`${CWD}/fixtures/dual-format-order.mjs`);
      const { imported, required, importedAgain, requiredAgain } =
        await load(order);

      expect(importedAgain).toBe(imported);
      expect(requiredAgain).toBe(required);
      expect(imported.default.format).toBe("esm");
      expect(required.format).toBe("cjs");
      expect(imported.default === required).toBe(false);
      expect(Object.isFrozen(imported.default)).toBe(true);
      expect(Object.keys(imported.default).sort()).toEqual(["format", "runs"]);
      expect(imported.default.runs()).toBe(1);
      expect(required.runs()).toBe(1);
    });
  }

  it("should import a CJS module with only a default export", async () => {
    const filename = `${CWD}/fixtures/default-only-export.cjs`;
    const mod = await import(filename);

    expect(Object.keys(mod)).toEqual(["default"]);
    expect(typeof mod.default).toBe("function");
    expect(mod.default()).toBe("hello from default");
  });

  for (const name of [
    "mjs-exports-regression",
    "mjs-commonjs-regression",
    "mjs-main-regression",
    "mjs-commonjs-regression/entry",
  ]) {
    it(`should load the mjs target of ${name} as ESM`, async () => {
      const filename = `${CWD}/fixtures/package-targets.mjs`;
      const { importPackage } = await import(filename);
      const mod = await importPackage(name);

      expect(mod.default).toEqual({ format: "esm" });
      expect(Object.isFrozen(mod.default)).toBe(true);
    });
  }

  it("should keep require conditions and explicit cjs targets as CommonJS", async () => {
    const filename = `${CWD}/fixtures/package-targets.mjs`;
    const { importPackage, requirePackage } = await import(filename);

    for (const name of ["mjs-exports-regression", "mjs-commonjs-regression"]) {
      expect(requirePackage(name)).toEqual({ format: "cjs" });
    }
    const mod = await importPackage("cjs-module-regression");
    expect(mod.default).toEqual({ format: "cjs" });
    expect(requirePackage("cjs-module-regression")).toEqual({ format: "cjs" });
  });

  it("regression testing for issue #1492", async () => {
    await import(`${CWD}/fixtures/test1492/index.js`);
  });
});
