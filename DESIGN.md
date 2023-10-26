## Design

Unlimited design will evolve at will. Suggestions are welcomed.

- Editor organization

  The editor will be split in two main parts:<br/>
     1. The core : a headless process/thread that handles all the files/computations
     <br/>
     1. The ui : another process/thread that presents the documents to the user<br/>
     <br/>

     These two threads communicate through standard channels (mpsc) using **Message**.<br/>

------

### Editor primitives

- **Message**<br/>
The **Message** main purpose is to encapsulate user inputs and internal communication between threads.<br/>
TODO(ceg): add timestamp info/ sequence / etc.<br/>


- **Buffer**<br/>
A **Buffer** represents a memory snapshot.<br/>
a **Buffer** can be created without any file attached to it.<br/>
a **Buffer** can be loaded from a file.<br/>
a **Buffer** can be saved to a file.<br/>
a **Buffer** can be detached from file.<br/>

- **BufferId** <br/>
A unique (unsigned 64 bits) integer that represents a given **Buffer** instance<br/>

- **File**<br/>
A regular on disk file

- **View**<br/>
a View contains:
   - BufferId and/or reference to **Buffer**
   - ViewId
   - InputMap
   - local Marks

- **Mode**<br/>

- **ModeContext**<br/>


- **Event**<br/>
Messages sent between the ui and the core


- **Codec**<br/>
The codec is responsible of the Buffer interpretation

    eg: TextCodec emits codepoints<br/>
    It convert from/to bytes/codepoints

- **CodecId**<br/>
a unique 64 bits integer that represents the codec.

- **CodecCtx**<br/>
A codec specific data structure

- **Mark**<br/>
A Mark represent a position in a Document<br/>
  * The **cursor** is a **Mark**
  * A mark can be fixed (it is up to the module managing the mark
  * A mark can be "local" to a given View  (wich is attached to a **Document**)
  * Marks can be "shared" by Document(s)

- **Selection**<br/>
There are 2 kinds of selection:<br/>
  * range selection : from one Mark to an other Mark
  * block selection (visual selection) : represents a rectangular selection depending on the displayed screen

- **InputMap**<br/>
The InputMap, will hold the input sequences the user must enter to trigger an action in the core thread.

  TODO(ceg): describe json format

------

### Behavior

We want the Ui (the view) to pilot the Core (model/controller):<br/>
- The Ui request a layout for a given View (+geometry hints)<br/>
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
- run selected ui main loop (in the main thread)<br/>

 **Ui main loop**<br>
 - the ui(s) received a screen from the core
 - and from there the ui can send input event to core
