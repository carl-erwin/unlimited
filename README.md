# unlimitED!


NOTE: THIS PROJECT JUST STARTED

**unlimitED!** is an experimental editor, and an excuse for me to learn the **Rust** language :-)<br/>
It is based on previous ideas/implementation done in one of my previous c++ project.<br/>

------

### Compiling

On a working Rust (stable) environment

```
git clone https://github.com/carl-erwin/unlimited
cd unlimited
cargo build
```

------

### Running

```
cargo run -- [FILE1] .. [FILEn]
```


------

### Goals

Ultimately remove all limitations found in common editors. (the file's size being the first)

------


### Contributing

More later, when there will be something useable...

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

- **Buffer**<br/>
A **Buffer** represents a memory snapshot of a given **File**.<br/>
**Buffer**(s) can be loaded from a file.<br/>
**Buffer**(s) can be saved to a file.<br/>
**Buffer**(s) can be dettached from file.<br/>
**Buffer**(s) can be created whitout any file.<br/>

- **BufferId** aka **_bid_**<br/>
An unsigned 64 bits integer that represent a given **Buffer** instance<br/>

- **Document**<br/>
A **Document** represents a **Buffer** PLUS it's configuration.<br/>
There is one and only one **Document** per **Buffer**.<br/>
An **Document** is always bound to a **Buffer**.

 It encapsulates:<br/>
A **Buffer**<br/>
the **View**(s)<br/>
the "shared" **Marks** (the cursor is a mark)<br/>
the font configuration<br/> (will be moved in the ui)
the selections<br/>
the internal regions<br/>

- **View**<br/>
a View contains:<br/>

 bid (BufferId)<br/>
 ViewId<br/>
 Codec<br/>
 CodecCtx<br/>
 InputMap<br/>
 local Marks<br/>

- **Event**<br/>
Messages sent between the ui and the core


- **Codec**<br/>
The codec is responible of the Buffer interpretation

TextCodec emits codepoints

- **CodecId**<br/>
a unique 64 bits integer that represents the codec.

- **CodecCtx**<br/>
A codec specific data structure

- **Mark**<br/>
A Mark represent a position in a Document<br/>
<br/>
The **cursor** is a **Mark**.<br/>
<br/>
Marks can be fixed (it is up to the module managing the marks).<br/>
Marks can be "local" to a given View  (wich is attached to a **Document**)<br/>
Marks can be "shared" by Document(s)<br/>

- **Selection**<br/>
there are 2 kinds of selection:<br/>
 * range selection : from one Mark to an other Mark
 * block selection (visual selection) : represents a rectangular selection depending on the displayed screen


- **InputMap**<br/>
The InputMap , will hold the action to be executed by the core.

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

### Startup sequences

- parse command line arguments<br/>
- store special options flags<br/>
- create/restore/merge file/Document list<br/>
- start core thread<br/>
- initialize sub systems<br/>
- setup modules/extensions<br/>
- start/run select ui main loop (in the main thread)<br/>

 **Ui main loop**
    - the ui(s) request the list of opened Documents
    - and from there the ui can request layout for a given Document
