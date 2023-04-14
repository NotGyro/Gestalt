use crate::resource::ResourceId;

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SubtextureDef {
	/// The entire base image file is used as this sprite
	WholeTexture,
	/// A static image taken from one cell of a sprite sheet.
	SpriteSheetCell {
		/// Which cell of the sprite sheet are we using?
		/// Whether this indexing a texture atlas or a texture array is determined at runtime, and
		/// may be dependent on options chosen by the user.
		/// So, in some cases, this will be an index into a texture array, and in some cases it
		/// is a spritesheet cell (column count is determined by the spritesheet's manifest file)
		cell_id: u16,
	},
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd)]
pub struct SpriteDef {
	pub base_texture: ResourceId,
	/// Determines which part (UV) of `base_texture` is used.
	/// Note that this applies to the *source* image and not to the exact layout of the bind group
	/// at runtime.
	pub portion: SubtextureDef,
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum BillboardType {
	/// Faces the camera in all directions
	Spherical,
	/// Faces the camera via Yaw but not Pitch or Roll.
	Pillar,
}

/// 2D-esque sprite in a 3D space.
#[derive(Clone, Debug)]
pub struct StaticBillboardDrawable {
	pub texture: SpriteDef,
	pub billboard_kind: BillboardType,
}

pub struct SpritesheetDef {
	/// How big is one cell in pixels?
	pub cell_sz: (u32, u32),
	/// How big is the entire sprite-sheet in pixels?
	pub image_sz: (u32, u32),
}
