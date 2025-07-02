//! Tests for storage implementations

use crate::memory::MemoryStorage;
use crate::models::*;
use crate::*;
use chrono::Utc;
use uuid::Uuid;

async fn create_test_storage() -> Box<dyn Storage<Error = StorageError>> {
    Box::new(MemoryStorage::new())
}

#[tokio::test]
async fn test_storage_initialization() {
    let storage = create_test_storage().await;

    // Should start empty
    let agents = storage.list_agents().await.unwrap();
    assert_eq!(agents.len(), 0);

    let tasks = storage.get_pending_tasks().await.unwrap();
    assert_eq!(tasks.len(), 0);
}

#[tokio::test]
async fn test_agent_crud_operations() {
    let storage = create_test_storage().await;

    // Create agent
    let agent = AgentModel {
        id: Uuid::new_v4().to_string(),
        name: "test-agent".to_string(),
        agent_type: "compute".to_string(),
        status: "idle".to_string(),
        capabilities: serde_json::json!(["neural", "analysis"]),
        config: serde_json::json!({"max_tasks": 5}),
        created_at: Utc::now().timestamp(),
        updated_at: Utc::now().timestamp(),
    };

    // Store
    storage.store_agent(&agent).await.unwrap();

    // Retrieve
    let retrieved = storage.get_agent(&agent.id).await.unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.name, agent.name);
    assert_eq!(retrieved.agent_type, agent.agent_type);

    // Update
    let mut updated_agent = retrieved;
    updated_agent.status = "running".to_string();
    storage.update_agent(&updated_agent).await.unwrap();

    let retrieved = storage.get_agent(&agent.id).await.unwrap().unwrap();
    assert_eq!(retrieved.status, "running");

    // Delete
    storage.delete_agent(&agent.id).await.unwrap();
    let retrieved = storage.get_agent(&agent.id).await.unwrap();
    assert!(retrieved.is_none());
}

#[tokio::test]
async fn test_task_operations() {
    let storage = create_test_storage().await;

    let agent_id = Uuid::new_v4().to_string();

    // Create multiple tasks
    for i in 0..5 {
        let task = TaskModel {
            id: Uuid::new_v4().to_string(),
            task_type: "compute".to_string(),
            status: if i < 3 { "pending" } else { "completed" }.to_string(),
            priority: i as i32,
            payload: serde_json::json!({"data": i}),
            assigned_to: if i >= 3 { Some(agent_id.clone()) } else { None },
            result: None,
            error: None,
            retry_count: 0,
            created_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
            completed_at: if i >= 3 {
                Some(Utc::now().timestamp())
            } else {
                None
            },
        };

        storage.store_task(&task).await.unwrap();
    }

    // Get pending tasks
    let pending = storage.get_pending_tasks().await.unwrap();
    assert_eq!(pending.len(), 3);

    // Get tasks by agent
    let agent_tasks = storage.get_tasks_by_agent(&agent_id).await.unwrap();
    assert_eq!(agent_tasks.len(), 2);

    // Claim a task
    let task_to_claim = &pending[0];
    let claimed = storage
        .claim_task(&task_to_claim.id, &agent_id)
        .await
        .unwrap();
    assert!(claimed);

    // Verify task is assigned
    let task = storage.get_task(&task_to_claim.id).await.unwrap().unwrap();
    assert_eq!(task.assigned_to, Some(agent_id.clone()));
}

#[tokio::test]
async fn test_event_logging() {
    let storage = create_test_storage().await;

    let agent_id = Uuid::new_v4().to_string();
    let start_time = Utc::now().timestamp();

    // Store various events
    let event_types = vec![
        "agent_started",
        "task_assigned",
        "task_completed",
        "agent_stopped",
    ];

    for (i, event_type) in event_types.iter().enumerate() {
        let event = EventModel {
            id: Uuid::new_v4().to_string(),
            event_type: event_type.to_string(),
            agent_id: Some(agent_id.clone()),
            task_id: if i == 1 || i == 2 {
                Some(Uuid::new_v4().to_string())
            } else {
                None
            },
            data: serde_json::json!({"index": i}),
            timestamp: start_time + i as i64,
        };

        storage.store_event(&event).await.unwrap();
    }

    // Get events by agent
    let agent_events = storage.get_events_by_agent(&agent_id, 10).await.unwrap();
    assert_eq!(agent_events.len(), 4);

    // Get events by type
    let task_events = storage
        .get_events_by_type("task_completed", 10)
        .await
        .unwrap();
    assert_eq!(task_events.len(), 1);

    // Get events since timestamp
    let recent_events = storage.get_events_since(start_time + 2).await.unwrap();
    assert_eq!(recent_events.len(), 2);
}

#[tokio::test]
async fn test_message_storage() {
    let storage = create_test_storage().await;

    let agent1 = Uuid::new_v4().to_string();
    let agent2 = Uuid::new_v4().to_string();

    // Store messages between agents
    for i in 0..5 {
        let message = MessageModel {
            id: Uuid::new_v4().to_string(),
            sender_id: if i % 2 == 0 {
                agent1.clone()
            } else {
                agent2.clone()
            },
            recipient_id: if i % 2 == 0 {
                agent2.clone()
            } else {
                agent1.clone()
            },
            message_type: "data".to_string(),
            payload: serde_json::json!({"content": format!("Message {}", i)}),
            read: false,
            timestamp: Utc::now().timestamp() + i,
        };

        storage.store_message(&message).await.unwrap();
    }

    // Get messages between agents
    let messages = storage
        .get_messages_between(&agent1, &agent2, 10)
        .await
        .unwrap();
    assert_eq!(messages.len(), 5);

    // Get unread messages
    let unread = storage.get_unread_messages(&agent1).await.unwrap();
    assert!(unread.len() > 0);

    // Mark message as read
    if let Some(first_unread) = unread.first() {
        storage.mark_message_read(&first_unread.id).await.unwrap();

        let still_unread = storage.get_unread_messages(&agent1).await.unwrap();
        assert_eq!(still_unread.len(), unread.len() - 1);
    }
}

#[tokio::test]
async fn test_metrics_storage() {
    let storage = create_test_storage().await;

    let agent_id = Uuid::new_v4().to_string();
    let start_time = Utc::now().timestamp();

    // Store various metrics
    for i in 0..10 {
        let metric = MetricModel {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.clone(),
            metric_type: if i < 5 { "cpu_usage" } else { "memory_usage" }.to_string(),
            value: (i as f64) * 10.0 + 50.0,
            tags: serde_json::json!({"host": "node1"}),
            timestamp: start_time + (i as i64) * 60,
        };

        storage.store_metric(&metric).await.unwrap();
    }

    // Get metrics by type
    let cpu_metrics = storage
        .get_metrics_by_agent(&agent_id, "cpu_usage")
        .await
        .unwrap();
    assert_eq!(cpu_metrics.len(), 5);

    // Get aggregated metrics
    let aggregated = storage
        .get_aggregated_metrics("cpu_usage", start_time, start_time + 600)
        .await
        .unwrap();
    assert!(aggregated.len() > 0);
}

#[tokio::test]
async fn test_list_agents_by_status() {
    let storage = create_test_storage().await;

    // Create agents with different statuses
    let statuses = vec!["idle", "running", "running", "stopped"];

    for (i, status) in statuses.iter().enumerate() {
        let agent = AgentModel {
            id: Uuid::new_v4().to_string(),
            name: format!("agent-{}", i),
            agent_type: "compute".to_string(),
            status: status.to_string(),
            capabilities: serde_json::json!([]),
            config: serde_json::json!({}),
            created_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
        };

        storage.store_agent(&agent).await.unwrap();
    }

    // Query by status
    let running_agents = storage.list_agents_by_status("running").await.unwrap();
    assert_eq!(running_agents.len(), 2);

    let idle_agents = storage.list_agents_by_status("idle").await.unwrap();
    assert_eq!(idle_agents.len(), 1);
}

#[tokio::test]
async fn test_storage_error_handling() {
    let storage = create_test_storage().await;

    // Try to get non-existent agent
    let result = storage.get_agent("non-existent-id").await.unwrap();
    assert!(result.is_none());

    // Try to update non-existent agent
    let agent = AgentModel {
        id: "non-existent".to_string(),
        name: "ghost".to_string(),
        agent_type: "phantom".to_string(),
        status: "invisible".to_string(),
        capabilities: serde_json::json!([]),
        config: serde_json::json!({}),
        created_at: Utc::now().timestamp(),
        updated_at: Utc::now().timestamp(),
    };

    // This should not panic, behavior depends on implementation
    let _ = storage.update_agent(&agent).await;
}

#[tokio::test]
async fn test_concurrent_storage_operations() {
    let storage = Arc::new(create_test_storage().await);

    let mut handles = vec![];

    // Spawn multiple concurrent operations
    for i in 0..10 {
        let storage_clone = storage.clone();
        let handle = tokio::spawn(async move {
            let agent = AgentModel {
                id: format!("agent-{}", i),
                name: format!("concurrent-agent-{}", i),
                agent_type: "worker".to_string(),
                status: "idle".to_string(),
                capabilities: serde_json::json!([]),
                config: serde_json::json!({}),
                created_at: Utc::now().timestamp(),
                updated_at: Utc::now().timestamp(),
            };

            storage_clone.store_agent(&agent).await
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Verify all agents were stored
    let agents = storage.list_agents().await.unwrap();
    assert_eq!(agents.len(), 10);
}

#[tokio::test]
async fn test_maintenance_operations() {
    let storage = create_test_storage().await;

    // These should not fail even if they're no-ops for memory storage
    storage.vacuum().await.unwrap();
    storage.checkpoint().await.unwrap();

    let size = storage.get_storage_size().await.unwrap();
    assert!(size >= 0);
}

use std::sync::Arc;
