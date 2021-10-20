Layer data is for things that will change *size* infrequently. Layer extensions are for things that will change size frequently such as larger-than-256 palettes.

* 8-bits / 1-byte version major (structure of literally everything that follows can potentially change based on this).
For major-version 0:
* 8-bits / 1-byte version minor
* 16-bits / 2-byte version build
* 32-bits / 4-byte end-of-header / start-of-data in bytes.
* 32-bits / 4-byte end-of-data / start-of-extensions in bytes.
* 32-bits / 4-byte reserved/padding
* LAYER ENTRIES. ALWAYS 256 of these. Slot corresponds to layer ID (so swapping order is less screwy).
 + 8-bits / 1-byte of "flags", lowest-7 bits of which get interpreted in a way specific to layer type.
  - First/highest bit is "present" i.e. if 0, this layer is unused in this chunk. Initialized to 0 when we're not "using" it i.e. both worldgen and players have not done anything with it.
 + 8-bits / 1-byte of order.
 + SO: By using LayerID (implicit, index here) to determine layer type, and passing that layer type our flags, you can calculate the exact size in bytes of the layer. (and also if it needs to reference a layer extension.) With order in the mix, and with knowledge of all layers in this chunk, you have 

End-of-header / start-of-data bytes into the file: 
*...Pages of actual data go here! Internal structure is up to the specific layer-type to determine...*
*(note that "internal structure is up to the specific layer" - there are only a few valid layer-types and they are determined at compile-time. So, version can remain consistent)*
*(also note that header is fixed-size - this might bite me in the ass later but it has a bunch of benefits here.)

End-of-data / start-of-extensions bytes into the file: 
* 16-bits / 2-byte "layer extension count" counting how many layers need extensions. 0 must mean none but layers 0-255 must also be representible - so 256 must be a valid value, so 16-bits.
* LAYER EXTENSION ENTRIES. (max 256, natch)
 + 8-bits / 1-byte of layer ID.
 + 24 bits / 3 bytes size (in bytes).
 + 32 bits / 4 bytes start/distancefromendoffile (in bytes).


Largest possible 1 layer I could ever imagine having: 262144 bytes.
8192*256 = 2097152
lol.
SMALLEST possible layer I could end up using: 4096 bytes + 512 palette...
extn at end of file - expected to change rapidly.
512-byte pages. 
the biggest possible layer is then:
33554432
262144

12-bit numpages... 4096 pages max. 
4096 pages*512 bytes per page  = 2097152 bytes
2097152
262144 ...max page size get? 
262144
Yesss score.
