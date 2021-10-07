unlimitED!


**unlimitED!** is an experimental editor, and an excuse to learn the **Rust** language.<br/>


---

### Features

- [x] basic utf-8 support
- [x] unlimited undo/redo
- [x] large file support

---

### Compiling

Minimum requirement : rust edition 2018 (https://www.rust-lang.org)

```
git clone https://github.com/carl-erwin/unlimited
 or
git clone https://gitlab.com/carl-erwin/unlimited

cd unlimited
cargo install --path .
```

---

### Running

by default cargo install puts the compiled program in **${HOME}/.cargo/bin**
```
unlimited [FILE1] .. [FILEn]
```

#### User Input Handling

  The keyboard/mouse shortcuts are currently hard-coded

  (see res/default_input_map.json)

---

### Goals

Ultimately remove all limitations found in common editors.

 - [x] handle large files
 - [ ] handle directories
 - [ ] handle very-long lines
 - [ ] provide a C API to handle basic primitives
 - [ ] an interactive "mode configurator" to customize a view

---

### Contributing

    You can submit your crazy ideas :-)
