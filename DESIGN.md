## Design

Unlimited design will evolve at will. Suggestions are welcomed.

- Editor organization

  The editor will be split in two main parts:<br/>
     1. The core : a headless process/thread that handles all the files/computations
     <br/>
     1. The ui : another process/thread that presents the documents to the user<br/>
     <br/>

     These two threads communicate through standard channels (mpsc) using a **Message**.<br/>

------

### Editor primitives

#### Message
The **Message** main purpose is to encapsulate user inputs/system events and it is used as the internal communication mechanism between threads.<br/>
TODO(ceg): add timestamp info/ sequence / etc.<br/>

#### Buffer
- represents an arbitrary sequence of bytes.<br/>
- can be created without any file attached to it.<br/>
- can be loaded from a file.<br/>
- can be saved to a file.<br/>
- can be detached from file.<br/>

#### BufferId
A unique (unsigned 64 bits) integer that represents a given **Buffer** instance<br/>

#### File
A regular on disk file

#### View

a View contains:
   - **BufferId** and/or reference to **Buffer**
   - **Modes**
   - **InputMap** stack

#### Mode
   - is a collection of action/data on View
   - for example the

#### ModeContext
   - the **View** can store a ModeContext (find by name)

#### Event
Event are payload found in **Message** sent between the ui and the core


#### Codec
It is responsible of the Buffer's data representation

eg: TextCodec emits codepoints<br/>
It convert from/to bytes/codepoints

#### CodecId
a unique 64 bits integer that represents the codec.

#### CodecCtx
A codec specific data structure

#### InputMap
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
  * notify a specific ui target (by view id)<br/>

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
