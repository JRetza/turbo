use anyhow::{Context, Result};
use turbo_tasks::{Value, Vc};
use turbopack_core::{
    asset::{Asset, AssetContent},
    chunk::{availability_info::AvailabilityInfo, ChunkableModule, ChunkingContext},
    ident::AssetIdent,
    module::Module,
    output::OutputAssets,
    reference::{ModuleReferences, SingleOutputAssetReference},
};

use super::chunk_item::ManifestChunkItem;
use crate::chunk::{EcmascriptChunkPlaceable, EcmascriptChunkingContext, EcmascriptExports};

#[turbo_tasks::function]
fn modifier() -> Vc<String> {
    Vc::cell("manifest chunk".to_string())
}

/// The manifest chunk is deferred until requested by the manifest loader
/// item when the dynamic `import()` expression is reached. Its responsibility
/// is to generate a Promise that will resolve only after all the necessary
/// chunks needed by the dynamic import are loaded by the client.
///
/// Splitting the dynamic import into a quickly generate-able manifest loader
/// item and a slow-to-generate manifest chunk allows for faster incremental
/// compilation. The traversal won't be performed until the dynamic import is
/// actually reached, instead of eagerly as part of the chunk that the dynamic
/// import appears in.
#[turbo_tasks::value(shared)]
pub struct ManifestChunkAsset {
    pub asset: Vc<Box<dyn ChunkableModule>>,
    pub chunking_context: Vc<Box<dyn EcmascriptChunkingContext>>,
    pub availability_info: AvailabilityInfo,
}

#[turbo_tasks::value_impl]
impl ManifestChunkAsset {
    #[turbo_tasks::function]
    pub fn new(
        asset: Vc<Box<dyn ChunkableModule>>,
        chunking_context: Vc<Box<dyn EcmascriptChunkingContext>>,
        availability_info: Value<AvailabilityInfo>,
    ) -> Vc<Self> {
        Self::cell(ManifestChunkAsset {
            asset,
            chunking_context,
            availability_info: availability_info.into_value(),
        })
    }

    #[turbo_tasks::function]
    pub(super) async fn chunks(self: Vc<Self>) -> Result<Vc<OutputAssets>> {
        let this = self.await?;
        Ok(this
            .chunking_context
            .chunk_group(Vc::upcast(this.asset), Value::new(this.availability_info)))
    }

    #[turbo_tasks::function]
    pub async fn manifest_chunks(self: Vc<Self>) -> Result<Vc<OutputAssets>> {
        let this = self.await?;
        Ok(this
            .chunking_context
            .chunk_group(Vc::upcast(self), Value::new(this.availability_info)))
    }
}

#[turbo_tasks::function]
fn manifest_chunk_reference_description() -> Vc<String> {
    Vc::cell("manifest chunk".to_string())
}

#[turbo_tasks::value_impl]
impl Module for ManifestChunkAsset {
    #[turbo_tasks::function]
    fn ident(&self) -> Vc<AssetIdent> {
        self.asset.ident().with_modifier(modifier())
    }

    #[turbo_tasks::function]
    async fn references(self: Vc<Self>) -> Result<Vc<ModuleReferences>> {
        let chunks = self.chunks();

        Ok(Vc::cell(
            chunks
                .await?
                .iter()
                .copied()
                .map(|chunk| {
                    Vc::upcast(SingleOutputAssetReference::new(
                        chunk,
                        manifest_chunk_reference_description(),
                    ))
                })
                .collect(),
        ))
    }
}

#[turbo_tasks::value_impl]
impl Asset for ManifestChunkAsset {
    #[turbo_tasks::function]
    fn content(&self) -> Vc<AssetContent> {
        todo!()
    }
}

#[turbo_tasks::value_impl]
impl ChunkableModule for ManifestChunkAsset {
    #[turbo_tasks::function]
    async fn as_chunk_item(
        self: Vc<Self>,
        chunking_context: Vc<Box<dyn ChunkingContext>>,
    ) -> Result<Vc<Box<dyn turbopack_core::chunk::ChunkItem>>> {
        let chunking_context =
            Vc::try_resolve_downcast::<Box<dyn EcmascriptChunkingContext>>(chunking_context)
                .await?
                .context(
                    "chunking context must impl EcmascriptChunkingContext to use \
                     ManifestChunkAsset",
                )?;
        Ok(Vc::upcast(
            ManifestChunkItem {
                chunking_context,
                manifest: self,
            }
            .cell(),
        ))
    }
}

#[turbo_tasks::value_impl]
impl EcmascriptChunkPlaceable for ManifestChunkAsset {
    #[turbo_tasks::function]
    fn get_exports(&self) -> Vc<EcmascriptExports> {
        EcmascriptExports::Value.cell()
    }
}
