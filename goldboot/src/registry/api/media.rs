// pub async fn download(path: web::Path<(String, String, String)>) -> Result<impl Responder> {
//     let (template, edition, arch) = path.into_inner();

//     match template.try_into()? {
//         TemplateBase::ArchLinux => Ok(web::Json(GetMediaResponse {
//             url: String::from(""),
//             checksum: None,
//         })),
//         _ => Err(actix_web::error::ErrorBadRequest("")),
//     }
// }
