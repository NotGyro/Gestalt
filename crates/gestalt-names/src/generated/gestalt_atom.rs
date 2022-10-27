pub type GestaltAtom = :: string_cache :: Atom < GestaltAtomStaticSet > ;
# [derive (PartialEq , Eq , PartialOrd , Ord)] pub struct GestaltAtomStaticSet ;
impl :: string_cache :: StaticAtomSet for GestaltAtomStaticSet { fn get () -> & 'static :: string_cache :: PhfStrSet { static SET : :: string_cache :: PhfStrSet = :: string_cache :: PhfStrSet { key : 10121458955350035957u64 , disps : & [(1u32 , 0u32)] , atoms : & ["foo",
"",
"bar"] , hashes : & [1462294212u32 , 4001824029u32 , 1914960281u32] } ;
& SET } fn empty_string_index () -> u32 { 1u32 } } pub const ATOM_GESTALTATOM__66_6F_6F : GestaltAtom = GestaltAtom :: pack_static (0u32) ;
pub const ATOM_GESTALTATOM_ : GestaltAtom = GestaltAtom :: pack_static (1u32) ;
pub const ATOM_GESTALTATOM__62_61_72 : GestaltAtom = GestaltAtom :: pack_static (2u32) ;
# [macro_export] macro_rules ! gestalt_atom { ("foo") => { $ crate :: gestalt_atom :: ATOM_GESTALTATOM__66_6F_6F } ;
("") => { $ crate :: gestalt_atom :: ATOM_GESTALTATOM_ } ;
("bar") => { $ crate :: gestalt_atom :: ATOM_GESTALTATOM__62_61_72 } ;
}