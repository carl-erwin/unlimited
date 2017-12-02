## Design

The Design will evolve at will. Suggestions are welcome.

- Editor organization

  The editor will be split in two parts:<br/>
     1. The core : a headless process/thread that handles all the files/computations
     2. The ui : another process/thread that presents the documents to the user

------

### Editor primitives

- **Document**<br/>
A **Document** represents a **Buffer** PLUS it's configuration.<br/>
There is one and only one **Document** per **Buffer**.<br/>
A **Document** is always bound to a **Buffer**.<br/>
A **Document** encapsulates:
  - a **Buffer**<br/>
  - the **View**(s)
  - the "shared" **Marks** (the cursor is a mark)
  - the font configuration (will be moved in the ui)
  - the selections
  - the internal regions



- **Buffer**<br/>
A **Buffer** represents a memory snapshot of a given **File**.<br/>
a **Buffer** can be loaded from a file.<br/>
a **Buffer** can be saved to a file.<br/>
a **Buffer** can be dettached from file.<br/>
a **Buffer** can be created whitout any file.<br/>

- **BufferId** <br/>
An unsigned 64 bits integer that represents a given **Buffer** instance<br/>

- **File**<br/>
A regular on disk file

- **View**<br/>
a View contains:
   - BufferId
   - ViewId
   - Codec
   - CodecCtx
   - InputMap
   - local Marks

- **Event**<br/>
Messages sent between the ui and the core


- **Codec**<br/>
The codec is responsible of the Buffer interpretation

    eg: TextCodec emits codepoints

- **CodecId**<br/>
a unique 64 bits integer that represents the codec.

- **CodecCtx**<br/>
A codec specific data structure

- **Mark**<br/>
A Mark represent a position in a Document<br/>
  * The **cursor** is a **Mark**
  * Marks can be fixed (it is up to the module managing the m
  * Marks can be "local" to a given View  (wich is attached to a **Document**)
  * Marks can be "shared" by Document(s)

- **Selection**<br/>
There are 2 kinds of selection:<br/>
  * range selection : from one Mark to an other Mark
  * block selection (visual selection) : represents a rectangular selection depending on the displayed screen


- **InputMap**<br/>
The InputMap, will hold the input sequences the user must enter to trigger an action in the core thread.

  ie: ctrl-s   -> save current document


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
 - the ui(s) request the list of opened Documents
 - and from there the ui can request layout for a given Document
