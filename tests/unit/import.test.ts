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

  it("should read the CJS default before named getters and retain collected keys", async () => {
    const filename = `${CWD}/fixtures/cjs-getter-order.cjs`;
    const mod = await import(filename);

    expect(mod.default.events).toEqual(["default", "first"]);
    expect(Object.keys(mod)).toEqual(["default", "first", "second"]);
    expect(mod.first).toBe(1);
    expect(mod.second).toBeUndefined();
    expect(Object.hasOwn(mod.default, "second")).toBe(false);
    expect(await import(filename)).toBe(mod);
    expect(mod.default.events).toEqual(["default", "first"]);
  });

  it("should enumerate a CJS proxy before reading its default and named exports", async () => {
    const filename = `${CWD}/fixtures/cjs-proxy-order.cjs`;
    const mod = await import(filename);
    const expected = [
      "ownKeys",
      "descriptor:named",
      "get:default",
      "get:named",
    ];

    expect(mod.default.events).toEqual(expected);
    expect(mod.named).toBe(42);
    expect(Object.keys(mod)).toEqual(["default", "named"]);
    expect(await import(filename)).toBe(mod);
    expect(mod.default.events).toEqual(expected);
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

  it("should preserve NUL characters in CJS export names without collisions", async () => {
    const filename = `${CWD}/fixtures/cjs-nul-export-names.cjs`;
    const mod = await import(filename);
    const expected = {
      "": 1,
      "\u0000": 2,
      abc: 3,
      "abc\u0000def": 4,
      "abc\u0000ghi": 5,
      "trailing\u0000": 6,
      "default\u0000suffix": 7,
      "π\u0000尾": 8,
    };

    expect(Object.keys(mod).sort()).toEqual(
      [...Object.keys(expected), "default"].sort()
    );
    expect(Object.keys(mod.default).sort()).toEqual(
      Object.keys(expected).sort()
    );
    for (const [key, value] of Object.entries(expected)) {
      expect(mod[key]).toBe(value);
      expect(mod.default[key]).toBe(value);
    }
  });

  it("should support static imports of CJS names containing NUL", async () => {
    const filename = `${CWD}/fixtures/cjs-nul-export-names.mjs`;
    const mod = await import(filename);

    expect(mod.leading).toBe(2);
    expect(mod.embedded).toBe(4);
    expect(mod.trailing).toBe(6);
    expect(mod.defaultPrefix).toBe(7);
  });

  for (const [name, expected] of [
    ["null", null],
    ["undefined", undefined],
  ] as const) {
    it(`should import a CJS ${name} export`, async () => {
      const filename = `${CWD}/fixtures/cjs-nullish/${name}.cjs`;
      const mod = await import(filename);

      expect(Object.keys(mod)).toEqual(["default"]);
      expect(mod.default).toBe(expected);
    });
  }

  for (const [name, expected] of [
    ["zero", 0],
    ["false", false],
    ["empty", ""],
  ] as const) {
    it(`should preserve the CJS wrapper for a ${name} default property`, async () => {
      const filename = `${CWD}/fixtures/cjs-nullish/${name}.cjs`;
      const mod = await import(filename);

      expect(mod.default).toEqual({ default: expected });
    });
  }

  it("regression testing for issue #1492", async () => {
    await import(`${CWD}/fixtures/test1492/index.js`);
  });
});
