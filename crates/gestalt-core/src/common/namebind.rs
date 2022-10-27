pub enum NamebindDomain { 
    /// Bind a ResourceId to a name
    Resource,
    /// Bind a WorldId to a name. 
    World,
    /// Bind a TileId to a name. Takes a WorldId as an argument since these bindings are per-world. 
    TileId(WorldId),
    // WIP - TileVariants are going to be akin to 
}