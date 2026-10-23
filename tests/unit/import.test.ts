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

  it("should import a CJS module with only a default export", async () => {
    const filename = `${CWD}/fixtures/default-only-export.cjs`;
    const mod = await import(filename);

    expect(Object.keys(mod)).toEqual(["default"]);
    expect(typeof mod.default).toBe("function");
    expect(mod.default()).toBe("hello from default");
  });

  it("should preserve CJS export names that are not binding identifiers", async () => {
    const filename = `${CWD}/fixtures/cjs-export-names.cjs`;
    const mod = await import(filename);
    const expected = {
      value: 1,
      class: 2,
      "a-b": 3,
      "": 4,
      0: 5,
      'a"b': 6,
      "a\\b": 7,
      "a\nb": 8,
      __llrt_export_0: 9,
      π: 10,
    };

    expect(mod.default).toEqual(expected);
    expect(Object.keys(mod).sort()).toEqual(
      [...Object.keys(expected), "default"].sort()
    );
    for (const [key, value] of Object.entries(expected)) {
      expect(mod[key]).toBe(value);
    }
  });

  it("should support static imports of CJS names requiring aliases", async () => {
    const filename = `${CWD}/fixtures/cjs-export-names.mjs`;
    const mod = await import(filename);

    expect(mod.value).toBe(1);
    expect(mod.reserved).toBe(2);
    expect(mod.hyphenated).toBe(3);
  });

  it("regression testing for issue #1492", async () => {
    await import(`${CWD}/fixtures/test1492/index.js`);
  });
});
