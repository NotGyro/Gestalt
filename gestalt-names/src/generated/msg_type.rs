pub type MsgType = :: string_cache :: Atom < MsgTypeStaticSet > ;
# [derive (PartialEq , Eq , PartialOrd , Ord)] pub struct MsgTypeStaticSet ;
impl :: string_cache :: StaticAtomSet for MsgTypeStaticSet { fn get () -> & 'static :: string_cache :: PhfStrSet { static SET : :: string_cache :: PhfStrSet = :: string_cache :: PhfStrSet { key : 3213172566270843353u64 , disps : & [(2u32 , 0u32)] , atoms : & ["",
"foo",
"bar"] , hashes : & [502452354u32 , 1329590298u32 , 1872190576u32] } ;
& SET } fn empty_string_index () -> u32 { 0u32 } } pub const ATOM_MSGTYPE_ : MsgType = MsgType :: pack_static (0u32) ;
pub const ATOM_MSGTYPE__66_6F_6F : MsgType = MsgType :: pack_static (1u32) ;
pub const ATOM_MSGTYPE__62_61_72 : MsgType = MsgType :: pack_static (2u32) ;
# [macro_export] macro_rules ! msg_type { ("") => { $ crate :: msgtype :: ATOM_MSGTYPE_ } ;
("foo") => { $ crate :: msgtype :: ATOM_MSGTYPE__66_6F_6F } ;
("bar") => { $ crate :: msgtype :: ATOM_MSGTYPE__62_61_72 } ;
}