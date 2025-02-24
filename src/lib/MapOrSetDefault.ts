export class MapOrSetDefault<K, T> extends Map<K, T> {
  #setDefault(key: K): T {
    const val = this.getDefault(key);
    this.set(key, val);
    return val;
  }
  constructor(public getDefault: (key: K) => T) {
    super();
  }
  update(key: K, from: (current: T) => T): T {
    // biome-ignore lint/style/noNonNullAssertion: duh
    const val = from(this.has(key) ? this.get(key)! : this.getDefault(key));
    this.set(key, val);
    return val;
  }
  getOrSetDefault(key: K): T {
    // biome-ignore lint/style/noNonNullAssertion: duh
    return this.has(key) ? this.get(key)! : this.#setDefault(key);
  }
}
