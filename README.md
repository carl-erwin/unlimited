# unlimitED!


**unlimitED!** is an experimental editor, and an excuse to learn the **Rust** language.<br/>
It is based on previous ideas/implementation done in one of my previous c++ project.<br/>

---

### Status

* the program is currently monothreaded
* basic utf-8 support
* Version 0.0.2 got unlimited undo/redo at the byte level, ie: every inserted character is added to the document's buffer log.
  This is the base api that will be used to implement other undo schemes (word, sentence, paragraph).<br/>
* Version 0.0.3 adds large file support<br/>


---

### Compiling

minimal requirement : rust stable (>= 1.17.0)

```
git clone https://github.com/carl-erwin/unlimited
cd unlimited
cargo install
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
  * **Delete next character**: ctrl-d
  * **Delete current character**: suppr
  * **Cut current line**: ctrl-k (from cursor to end of line)
  * **Paste previous killed line**: ctrl-y
  * **Save**: ctrl-s
  * **Quit**: ctrl-q (will quit without saving changes for now)
  * **Goto beginning of line**: ctrl-a
  * **Goto end of line**: ctrl-e
  * **Page Up**: scroll up on screen at time
  * **Page Down**: scroll down on screen at time
  * **Mouse left button**: move cursor to clicked area

---

### Goals

Ultimately remove all limitations found in common editors. (the file's size being the first)

---

### Contributing

    You can submit your crazy ideas :-)
