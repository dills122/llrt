module.exports = {
  "": 1,
  "\u0000": 2,
  abc: 3,
  "abc\u0000def": 4,
  "abc\u0000ghi": 5,
  "trailing\u0000": 6,
  "default\u0000suffix": 7,
  "π\u0000尾": 8,
};
