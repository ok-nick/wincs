use core::str;
use std::{fs, future::Future, path::Path, pin::Pin};

use anyhow::Context;
use cloud_filter::{
    error::{CResult, CloudErrorKind},
    filter::{info, ticket, AsyncBridge, Filter},
    metadata::Metadata,
    placeholder_file::PlaceholderFile,
    request::Request,
    root::{
        Connection, HydrationType, PopulationType, SecurityId, Session, SyncRootId,
        SyncRootIdBuilder, SyncRootInfo,
    },
    utility::WriteAt,
};
use libtest_mimic::Failed;
use nt_time::FileTime;

const ROOT_PATH: &str = "C:\\async_filter_test";

struct MemFilter;

impl Filter for MemFilter {
    async fn fetch_data(
        &self,
        request: Request,
        ticket: ticket::FetchData,
        info: info::FetchData,
    ) -> CResult<()> {
        let path = unsafe { str::from_utf8_unchecked(request.file_blob()) };
        println!("fetch_data: path: {path:?}");

        let content = match path.as_ref() {
            "test1.txt" | "dir1\\test2.txt" => path,
            _ => Err(CloudErrorKind::InvalidRequest)?,
        };

        if info.required_file_range() != (0..content.len() as u64) {
            Err(CloudErrorKind::InvalidRequest)?;
        }

        ticket.write_at(content.as_bytes(), 0).unwrap();

        Ok(())
    }

    async fn fetch_placeholders(
        &self,
        request: Request,
        ticket: ticket::FetchPlaceholders,
        _info: info::FetchPlaceholders,
    ) -> CResult<()> {
        let path = request.path();
        let relative_path = path.strip_prefix(ROOT_PATH).unwrap();
        println!("fetch_placeholders: path: {path:?}, relative path: {relative_path:?}");

        let now = FileTime::now();
        let mut placeholders = match relative_path.to_string_lossy().as_ref() {
            "" => vec![
                PlaceholderFile::new("dir1")
                    .mark_in_sync()
                    .metadata(Metadata::directory().created(now).written(now).size(0))
                    .blob("dir1".into()),
                PlaceholderFile::new("test1.txt")
                    .has_no_children()
                    .mark_in_sync()
                    .metadata(
                        Metadata::file()
                            .created(now)
                            .written(now)
                            .size("test1.txt".len() as _),
                    )
                    .blob("test1.txt".into()),
            ],
            "dir1" => vec![PlaceholderFile::new("test2.txt")
                .has_no_children()
                .mark_in_sync()
                .metadata(
                    Metadata::file()
                        .created(now)
                        .written(now)
                        .size("dir1\\test2.txt".len() as _),
                )
                .blob("dir1\\test2.txt".into())],
            _ => Err(CloudErrorKind::InvalidRequest)?,
        };

        ticket.pass_with_placeholder(&mut placeholders).unwrap();
        Ok(())
    }
}

fn init() -> anyhow::Result<(
    SyncRootId,
    Connection<AsyncBridge<MemFilter, impl Fn(Pin<Box<dyn Future<Output = ()>>>)>>,
)> {
    let sync_root_id = SyncRootIdBuilder::new("sync_filter_test_provider")
        .user_security_id(SecurityId::current_user().context("current_user")?)
        .build();

    if !sync_root_id.is_registered().context("is_registered")? {
        sync_root_id
            .register(
                SyncRootInfo::default()
                    .with_display_name("Sync Filter Test")
                    .with_hydration_type(HydrationType::Full)
                    .with_population_type(PopulationType::Full)
                    .with_icon("%SystemRoot%\\system32\\charmap.exe,0")
                    .with_version("1.0.0")
                    .with_recycle_bin_uri("http://cloudmirror.example.com/recyclebin")
                    .context("recycle_bin_uri")?
                    .with_path(ROOT_PATH)
                    .context("path")?,
            )
            .context("register")?
    }

    let connection = Session::new()
        .connect_async(ROOT_PATH, MemFilter, move |f| {
            futures::executor::block_on(f)
        })
        .context("connect")?;

    Ok((sync_root_id, connection))
}

pub fn test() -> Result<(), Failed> {
    if !Path::new(ROOT_PATH).try_exists().context("exists")? {
        fs::create_dir(ROOT_PATH).context("create root dir")?;
    }

    let (sync_root_id, connection) = init().context("init")?;

    crate::test_list_folders(ROOT_PATH);
    crate::test_read_file(ROOT_PATH);

    drop(connection);
    sync_root_id.unregister().context("unregister")?;

    fs::remove_dir_all(ROOT_PATH).context("remove root dir")?;

    Ok(())
}
