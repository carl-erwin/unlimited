# unlimitED!


**unlimitED!** is an experimental editor, and an excuse to learn the **Rust** language.<br/>
It is based on previous ideas/implementation done in one of my previous c++ project.<br/>

---

### Features

* basic utf-8 support
* unlimited undo/redo at the byte level, ie: every inserted character is added to the document's buffer log.
  This will be used to implement other undo schemes (word, sentence, paragraph).<br/>
* large file support<br/>

---

### Compiling

minimum requirement : rust edition 2018 (https://www.rust-lang.org)

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

#### Keyboard mapping (hard-coded)

  * **F1**: select previous file
  * **F2**: select next file
  * **Any character**: insert the character
  * **Undo**: ctrl-u
  * **Redo**: ctrl-r
  * **Delete previous character**: backspace
  * **Delete current character**: ctrl-d
  * **Delete current character**: suppr
  * **Cut current line**: ctrl-k (from cursor to end of line)
  * **Paste previous killed line**: ctrl-y
  * **Save**: ctrl-s
  * **Quit**: ctrl-q (will quit without saving changes for now)
  * **Goto beginning of line**: ctrl-a
  * **Goto end of line**: ctrl-e
  * **Page Up**: scroll up on screen at time
  * **Page Down**: scroll down on screen at time
  * **Goto beginning of file**: ctrl+<
  * **Goto end of file**: ctrl+>
  * **Mouse left button**: move cursor to clicked area
  * **Center arround mark/cursor**: ctrl+l

---

### Goals

Ultimately remove all limitations found in common editors. (the file's size being the first)

---

### Contributing

    You can submit your crazy ideas :-)
