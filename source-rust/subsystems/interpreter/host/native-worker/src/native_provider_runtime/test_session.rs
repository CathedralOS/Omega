//! Integration fixture for malformed handles, worker faults and persistent
//! mappings. It uses the real bounded process and published adapter loader.

use super::*;
use image_emission::NativeForeignAdapterImage;
use native_provider_sponsor::test_support::FixtureWorker;
use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
    rc::Rc,
};

struct FixtureSession {
    worker: Rc<RefCell<FixtureWorker>>,
    directory: std::path::PathBuf,
}
impl NativeProviderSession for FixtureSession {
    fn home_directory(&self) -> &Path {
        &self.directory
    }
    fn release_home_directory(&mut self) {
        self.worker.borrow_mut().release_home_directory();
    }

    fn install(
        &mut self,
        _: &PublishedForeignAdapterIdentity,
        _: u64,
        _: NativeAdapterStorage<'_>,
    ) -> io::Result<Vec<u8>> {
        Err(io::Error::other(
            "fixture installation requires its actual product",
        ))
    }
    fn exchange(&mut self, operation: NativeProviderOperation<'_>) -> io::Result<Vec<u8>> {
        self.worker.borrow_mut().exchange(operation.as_bytes())
    }
}

pub(crate) struct TestProviderProcess {
    process: NativeProviderProcess,
    worker: Rc<RefCell<FixtureWorker>>,
}
impl Deref for TestProviderProcess {
    type Target = NativeProviderProcess;
    fn deref(&self) -> &Self::Target {
        &self.process
    }
}
impl DerefMut for TestProviderProcess {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.process
    }
}
impl TestProviderProcess {
    pub(crate) fn launch(command: Command, timeout: Duration) -> Result<Self, NativeProviderError> {
        let (child, worker) = native_provider_sponsor::test_support::spawn(command, timeout)?;
        let directory = worker.home_directory().to_owned();
        let worker = Rc::new(RefCell::new(worker));
        let process = NativeProviderProcess::from_sponsor(
            child,
            Box::new(FixtureSession {
                worker: Rc::clone(&worker),
                directory,
            }),
            timeout,
        )?;
        Ok(Self { process, worker })
    }
    pub(crate) fn install(
        &mut self,
        product: &NativeForeignAdapterImage,
        storage: &[NativeParameterStorage],
    ) -> Result<NativeAdapterId, NativeProviderError> {
        self.install_with_result(product, storage, NativeResultStorage::Value)
    }
    pub(crate) fn install_with_result(
        &mut self,
        product: &NativeForeignAdapterImage,
        storage: &[NativeParameterStorage],
        result: NativeResultStorage,
    ) -> Result<NativeAdapterId, NativeProviderError> {
        self.process.install_admitted(
            &product.runtime_identity(),
            storage,
            result,
            |_, ordinal, geometry| self.worker.borrow_mut().install(product, ordinal, geometry),
        )
    }
    pub(crate) fn close(self) -> Result<(), NativeProviderError> {
        self.process.close()
    }
}
