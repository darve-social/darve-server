/*pub async fn mw_htmx_transformer(res: Response) -> Response {
    println!("->> {:<12} - mw_response_transformer:", "HTMX TRANSFORMER");
    let is_err = res.status().is_server_error() || res.status().is_client_error();

    let htmx_response_header = "vary";
    match (is_err, res.headers().get(htmx_response_header)) {
        (true, Some(_)) => {
            let (mut parts, body) = res.into_parts();

            let bBytes = body::to_bytes(body, 9999).await.ok();
            if let Some(bBytes) = bBytes {
                let mut bStr = String::from_utf8_lossy(bBytes.to_vec().as_slice()).to_string();
                let mut ret_html = "<div>".to_string();
                let json: Option<ErrorResponseBody> = serde_json::from_str(bStr.as_str()).ok();
                if let Some(err) = json {
                    bStr = err.get_err();
                    let found = bStr.find("\n");
                    let lines =bStr.split("\n");
                    if found.is_some() {
                        ret_html+="<ul>";
                        for line in lines.into_iter() {
                            ret_html+=format!("<li>{line}</li>").as_str();
                        }
                        ret_html+="</ul>"
                    }else {
                        ret_html +=bStr.as_str();
                    }

                }
                ret_html += "</div>";
                    parts.headers.remove("content-length");
                    return Response::from_parts(parts, ret_html.into());
            } else {
                return Response::from_parts(parts, "Error transforming response".into());
            }
        }
        (_, _) => res
    }
}*/
