Notes
====
Todo: 
----
- [x] File I/O (don't overdo it, instead get something minimal without all of the self-describing stuff but make sure that it can be COMPOSED later.)
- [ ] Change all of the get position upper / lower functions to return a value rather than an option, 
and specify that requesting these values from a pseudo-infinite world gets you the bounding box of ... something (decide)
- [x] Decide whether the "World" object implements the VoxelStorage trait or not. _Answer: No._
- [ ] Multiple chunks (bigworld), loading / unloading around the player.
- [ ] Start on Lua scripting.
- [ ] Iterators over VoxelStorages

----

I thought about quitting Rust briefly. Long story short - data-oriented design would
* Make things faster because CPU cache would hate us less
* Make the whole thing less likely to piss off the borrow checker (spiderweb-like class hierarchies are the problem).
So, let's take a different pattern here. 

----

materialart.rs contains a decent pattern for doing run-time downcasting -- look back to that in the future.
Should be possible to make it easier through macros.

World format
-----
All of this is for later, the representation inside the program is more important right now. 

World layer exists as a file format / loader concept, but, mostly, not as an in-engine concept - not until we want modders to make their own world-parallel layers from a scripting language, that is.
We need a low-overhead way of specifying "extra" world layers, optional ones, for runtime-created world layers. i.e. it doesn't need to exist in the schema.
I guess modders could also ask for required world layers but I'm going to just guess right now that it would probably not be so performant and be a pain to implement.

Lighting: For future reference
-----
3D chunks in Minecraft clones generally have the one major hurdle of global light / sunlight. 
The idea that just struck me - and this would be imperfect and create artifacts, but it'd work without being horrendously slow - is to record a density value (specifically, what proportion of
cells in each chunk are filled with non-translucent tiles) for each chunk and cache it for each unloaded chunk. 

Map File Format, Abstract
-----
World folder
	Every schema definition file for every schema ever used in this map
	Map worldgen record
		Packed array of elements
			128-byte ASCII string describing the version used to generate this chunk in the world. -- Maybe not! Maybe this should be part of the voxelevent log.
	Map file
		Name of Schema
		Version of Schema
		Layer 0 Index elements (index entries for pages, packed array for constant time lookups)
				64-bit Location of data
				Length of data
				Flags
		Layer 1 Index Elements
		Layer 2 Index Elements...
		Layer 3 Index Elements...
		Layer 0 content
			File header
				Length of header
				Position of second page (0 for none)
				Length of data
			Header
			Data
		Layer 1 content
		Layer 2 content...
		Layer 3 content...
		Layer 1 Page 2
		Layer 3 Page 2
		Layer 8 Page 4
		Layer 4 Page 3
		...
