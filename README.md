# unlimitED!


NOTE: THIS PROJECT JUST STARTED

**unlimitED!** is an experimental editor, and an excuse for me to learn the **Rust** language :-)<br/>
It is based on previous ideas/implementation done in one of my previous c++ project.<br/>

------

### Compiling

```
git clone https://github.com/carl-erwin/unlimited
cd unlimited
cargo build
```

------

### Goals

Being simple and ultimately remove all limitations found in common editors. (the file's size beign the first)

------


### Contributing

More later, when there will be some code...

------

### Design

The Design will evolve at will. Suggestions are welcome.

- Editor organization

  The editor will be split in two parts:<br/>
     1. The core : a headless process/thread that handles all the files/computations
     2. The ui : another process/thread that presents the files to the user

------

### Editor primitives

- **File**<br/>
A regular on disk file

- **ByteBuffer**<br/>
A **ByteBuffer** represents a memory snapshot of a given **File**.<br/>
**ByteBuffer**(s) can be loaded from a file.<br/>
**ByteBuffer**(s) can be saved to a file.<br/>
**ByteBuffer**(s) can be dettached from file.<br/>
**ByteBuffer**(s) can be created whitout any file.<br/>

- **ByteBufferId** aka **_bid_**<br/>
An unsigned 64 bits integer that represent a given **ByteBuffer** instance<br/>

- **EditorBuffer**<br/>
An **EditorBuffer** represents a **ByteBuffer** and it's configuration.<br/>
There is one and only one **EditorBuffer** per **ByteBuffer**.<br/>
An **EditorBuffer** is always bound to a **ByteBuffer**.

 It encapsulates:<br/>
A **ByteBuffer**<br/>
the **EditorView**(s)<br/>
the "shared" **Marks** (the cursor is a mark)<br/>
the font configuration<br/> (will be moved in the ui)
the selections<br/>
the internal regions<br/>

- **EditorView**<br/>
an EditorView contains:<br/>

 bid (ByteBufferId)<br/>
 ViewId<br/>
 Codec<br/>
 CodecCtx<br/>
 EditorInputMap<br/>
 local Marks<br/>

- **EditorEvent**<br/>
Messages sent between the ui and the core


- Codec<br/>
The codec is responible of the ByteBuffer interpretation

TextCodec emits codepoints

- CodecId<br/>
a unique 64 bits integer that represents the codec.

- CodecCtx<br/>
A codec specific data structure

- Mark<br/>
A Mark represent a position in an EditorBuffer<br/>
<br/>
The **cursor** is a **Mark**.<br/>
<br/>
Marks can be fixed (it is up to the module managing the marks).<br/>
Marks can be "local" to a given EditorView  (wich is attached to an **EditorBuffer**)<br/>
Marks can be "shared" by EditorBuffer(s)<br/>

- **Selection**<br/>
there are 2 kinds of selection:<br/>
 * range selection : from one Mark to an other Mark
 * block selection (visual selection) : represents a rectangular selection depending on the displayed screen


- **EditorInputMap**<br/>
The EditorInputMap , will hold the action to be executed by the core.

------

### Behavior

I want the Ui (the view) to pilot the Core (model/controller):<br/>
- The Ui request a layout for a given View.<br/>
- The Core can send notifications:
  * popup messages (+geometry hints)<br/>
  * msg + yes/no   (quit)<br/>
  * task status (unknown/running/paused/terminated/unresponsive)<br/>
  * notifiy a specific ui target (by view id)<br/>

------

### Startup

    - parse command line arguments
      *) store special options flags
    - init sub systems
    - setup modules/plugins
    - create/restore/merge file/buffer list
    - start core thread
    - start/run select ui main loop (in the main thread)

    [Ui main loop]
    - the ui(s) request the list of opened buffers
    - and from there th ui can request layout for a given buffer

    ------
