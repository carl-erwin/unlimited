**unlimitED!** is an experimental editor, and an excuse to learn the [**Rust**](https://www.rust-lang.org) language.<br/>
<br/>
**Warning: It is in the alpha stage and not suited for daily work.**

---

![Alt text](/res/img/unlimited-screenshot.png "screenshot")

### Features

- [x] basic utf-8 support
- [x] unlimited undo/redo
- [x] large file support
- [x] simple search
- [x] goto line
- [x] highlight keywords (hack, this is not syntax highlighting)
- [x] mouse selection

---

### Compiling

Minimum requirement : [**Rust**](https://www.rust-lang.org) edition 2018

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
 - [ ] an interactive "configurator" mode to customize a view

---

### Contributing

    You can submit your crazy ideas :-)
