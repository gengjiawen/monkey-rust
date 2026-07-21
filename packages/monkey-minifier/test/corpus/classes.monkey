class Box {
  constructor(value) { this.value = value; }
  get() { this.value }
}
let box = new Box(42);
box.get();
