const events = [];
module.exports = new Proxy(() => {}, {
  ownKeys() {
    events.push("ownKeys");
    return ["named"];
  },
  getOwnPropertyDescriptor() {
    events.push("descriptor:named");
    return { configurable: true, enumerable: true, value: 42 };
  },
  get(target, key) {
    events.push(`get:${String(key)}`);
    return key === "default" ? { events } : 42;
  },
});
