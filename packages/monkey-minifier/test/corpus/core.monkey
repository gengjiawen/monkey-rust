let makeAdder = fn(value) {
  fn(extra) { value + extra }
};
let addTwo = makeAdder(2);
puts(addTwo(40));
