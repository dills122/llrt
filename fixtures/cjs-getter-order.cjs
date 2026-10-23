const events = [];
const value = () => {};
Object.defineProperty(value, "events", { value: events });
Object.defineProperty(value, "default", {
  enumerable: true,
  get() {
    events.push("default");
    return value;
  },
});
Object.defineProperty(value, "first", {
  enumerable: true,
  get() {
    events.push("first");
    delete value.second;
    return 1;
  },
});
value.second = 2;
module.exports = value;
