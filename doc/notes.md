Notes
====

All of this is for later, the representation inside the program is more important right now. 

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
			128-byte ASCII string describing the version used to generate this chunk in the world.
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