Vertex format for blocky chunk rendering.

Important: The smallest variable we can hand to each vertex is 32 bits anyway.

Under 2DTextureArray: Only 2 bits required for UV - Is our X a 1 or a 0, and, is our Y a 1 or a 0.
Texture atlas would get more complicated.

2 bits assumed for UV, 30 bits remaining.

How many textures do we think we can have?

Common values for GL_MAX_ARRAY_TEXTURE_LAYERS seem to be 8192 and 2048.
That gives us either 11 bits for texture ID or 13 bits for texture ID, respectively.
However, 12 bits gives us a very respectable 4096 while fitting very snugly.

5 bits per dimension = integer position of 0 to 31, 15 bits total. 
6 bits per dimension * = integer position of 0 to 63, 18 bits total. Neatly fits values of 0 to 16 and 0 to 32, both likely chunk sizes.
7 bits per dimension = integer position of 0 to 127, 21 bits total.
8 bits per dimension * = integer position of 0 to 256, 24 bits total. Very nice.

One scenario: 
Pos(start):     Values (in this order)    Remaining space:    
0 bits                                    32 bits
0 bits           6 bits position X        26 bits
6 bits           6 bits position Y        20 bits
12 bit           6 bits position Z        14 bits
18 bits          12 bits textureID        2 bits
30 bits          1 bit U                  1 bits
31 bits          1 bit V                  1 bits