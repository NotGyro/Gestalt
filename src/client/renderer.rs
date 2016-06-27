pub mod voxel;
use voxel::material::MaterialID;

/* I'm not actually sure how to structure the renderers in Gestalt. 
I suppost I'll start with one that renders a single chunk and 
then discover what to do next iteratively from there.
*/

pub trait VoxelRenderer {
    /* Arguments: 
    mat: What material are we setting the art for? 
    art: The material art we're registering and associating here.*/
	pub fn reg_material_art(&mut self, mat : MaterialID, art : MaterialArt);
}
