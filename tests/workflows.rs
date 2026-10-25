mod common;

use axum::http::StatusCode;
use common::{assert_error_envelope, delete, fixture_home, get, json_request, send_json, test_app};
use serde_json::json;

#[tokio::test]
async fn workflows_are_created_inside_a_project() {
    let (_directory, app) = test_app();

    // The retired flat routes explain where creation moved.
    let (status, listing) = send_json(&app, get("/workflows")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listing, json!([]));

    let (status, body) = send_json(
        &app,
        json_request("POST", "/workflows", &json!({"name": "x"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_error_envelope(&body, "invalid_request");

    // Creation through the project-scoped route.
    let home = fixture_home(&app).await;

    // Create a case first so the workflow can reference it.
    let (status, _) = send_json(
        &app,
        json_request(
            "POST",
            &format!("/projects/{home}/test_cases"),
            &json!({
                "testCaseId": "TC-001",
                "title": "Test case",
                "expectedResult": "It works"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, created) = send_json(
        &app,
        json_request(
            "POST",
            &format!("/projects/{home}/workflows"),
            &json!({
                "name": "release-smoke",
                "steps": [{"testCaseId": "TC-001"}]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["id"], "release-smoke.json");

    // Listing through the project.
    let (status, listing) = send_json(&app, get(&format!("/projects/{home}/workflows"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listing, json!(["release-smoke.json"]));
}

#[tokio::test]
async fn workflows_support_the_full_crud_lifecycle() {
    let (_directory, app) = test_app();
    let home = fixture_home(&app).await;

    // Create a case first.
    let (status, _) = send_json(
        &app,
        json_request(
            "POST",
            &format!("/projects/{home}/test_cases"),
            &json!({
                "testCaseId": "TC-001",
                "title": "Test case",
                "expectedResult": "It works"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Create
    let (status, created) = send_json(
        &app,
        json_request(
            "POST",
            &format!("/projects/{home}/workflows"),
            &json!({
                "name": "smoke",
                "steps": [{"testCaseId": "TC-001"}]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap();

    // Read
    let (status, stored) = send_json(&app, get(&format!("/workflows/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stored["name"], "smoke");

    // Update
    let (status, _updated) = send_json(
        &app,
        json_request(
            "PUT",
            &format!("/workflows/{id}"),
            &json!({"name": "smoke-v2"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Read again to verify
    let (status, stored) = send_json(&app, get(&format!("/workflows/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stored["name"], "smoke-v2");

    // Duplicate through the flat route, then delete the copy through it.
    let (status, duplicate) = send_json(
        &app,
        json_request(
            "POST",
            &format!("/workflows/{id}/duplicate"),
            &json!({"newId": "smoke-copy.json"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{duplicate}");
    let copy_id = duplicate["id"].as_str().unwrap();
    let (status, _) = send_json(&app, delete(&format!("/workflows/{copy_id}"))).await;
    assert_eq!(status, StatusCode::OK);

    // Materialise an empty workflow into a run from its project. The run
    // route needs the project snapshot and the provenance stamp even when the
    // plan has no steps.
    let (status, empty) = send_json(
        &app,
        json_request(
            "POST",
            &format!("/projects/{home}/workflows"),
            &json!({"name": "empty", "steps": []}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{empty}");
    let empty_id = empty["id"].as_str().unwrap();
    let (status, run) = send_json(
        &app,
        json_request(
            "POST",
            &format!("/projects/{home}/workflows/{empty_id}/run"),
            &json!({"name": "empty-run"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{run}");

    // Delete the empty workflow through the flat route.
    let (status, _) = send_json(&app, delete(&format!("/workflows/{empty_id}"))).await;
    assert_eq!(status, StatusCode::OK);

    // Delete through the project-scoped route
    let (status, _) = send_json(&app, delete(&format!("/projects/{home}/workflows/{id}"))).await;
    assert_eq!(status, StatusCode::OK);

    // Gone
    let (status, _) = send_json(&app, get(&format!("/workflows/{id}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
