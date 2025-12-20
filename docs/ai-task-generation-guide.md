# AI-Powered Task Generation Guide

## Overview

This feature allows users to describe their tasks in natural language and have an AI assistant convert them into structured TODO items that are automatically inserted into the project markdown file. The system maintains the existing file structure while intelligently organizing new tasks into appropriate sections.

## Architecture

The feature follows a clean separation of concerns pattern with three main components:

1. **AITaskGenerator Service**: Handles communication with AI APIs and formats prompts/responses
2. **TaskInputComponent**: UI component for user interaction
3. **TaskInsertionLogic**: Extensions to ProjectStateEditor for intelligent task placement

These components integrate with our existing reactive state management approach and maintain clear boundaries between UI, business logic, and data persistence.

## Component Responsibilities

### AITaskGenerator Service

```typescript
// src/lib/AITaskGenerator.ts
import { invoke } from "@tauri-apps/api/core";
import type { ProjectMarkdown } from "./ProjectStateEditor";

export interface AIGeneratedTask {
  text: string;
  heading?: string;
  priority?: number;
  subTasks?: AIGeneratedTask[];
}

export class AITaskGenerator {
  constructor(private apiKey: string) {}

  /**
   * Generate tasks from a natural language description
   */
  async generateTasks(description: string, existingHeadings: string[]): Promise<AIGeneratedTask[]> {
    try {
      // Build the system prompt with context about existing project structure
      const systemPrompt = this.buildSystemPrompt(existingHeadings);
      
      // Invoke the AI API through Tauri (recommended for key security)
      const response = await invoke<AIGeneratedTask[]>("generate_ai_tasks", {
        description,
        systemPrompt,
        apiKey: this.apiKey
      });
      
      return response;
    } catch (error) {
      console.error("AI Task generation failed:", error);
      throw new Error(`Failed to generate tasks: ${error}`);
    }
  }

  private buildSystemPrompt(existingHeadings: string[]): string {
    return `You are a task organization assistant. Convert natural language descriptions into 
well-structured TODO items. Follow these rules:

1. Each task should be atomic and actionable
2. Group related tasks under appropriate headings
3. Consider using these existing project headings when relevant: ${existingHeadings.join(", ")}
4. Return tasks in a structured format with text and optional heading
5. Tasks should be specific and clear
6. Prioritize tasks implicitly based on their logical sequence`;
  }
}
```

### ProjectStateEditor Extensions

Extend the existing ProjectStateEditor with intelligent task insertion capabilities:

```typescript
// src/lib/ProjectStateEditor.ts (addition)
import type { AIGeneratedTask } from "./AITaskGenerator";

export namespace ProjectStateEditor {
  // ... existing code ...

  /**
   * Intelligently insert AI-generated tasks into the existing markdown structure
   */
  export function insertAIGeneratedTasks(
    originalContent: string,
    tasks: AIGeneratedTask[]
  ): string {
    // Parse the original content first
    const projectFile = parse(originalContent);
    const parsedMarkdown = projectFile.markdown;
    
    // Get existing headings for better organization
    const existingHeadings = extractHeadings(parsedMarkdown);
    const headingToItemsMap = groupByHeading(parsedMarkdown);
    
    // Process each AI-generated task
    for (const task of tasks) {
      const targetHeading = findBestHeadingMatch(task.heading, existingHeadings) || 
                            task.heading || 
                            (existingHeadings.length > 0 ? existingHeadings[0] : null);
      
      // Create a new heading if needed
      if (targetHeading && !existingHeadings.includes(targetHeading)) {
        parsedMarkdown.push({ 
          type: "unrecognized", 
          markdown: "" 
        });
        parsedMarkdown.push({ 
          type: "heading", 
          level: 1, 
          text: targetHeading 
        });
        existingHeadings.push(targetHeading);
        headingToItemsMap.set(targetHeading, []);
      }
      
      // Create the task object
      const newTask: ProjectMarkdown = {
        type: "task",
        complete: false,
        name: task.text,
        details: null,
        prefix: "- "
      };
      
      // Insert task in the right place
      if (targetHeading) {
        // Find where to insert within the section
        const sectionItems = headingToItemsMap.get(targetHeading) || [];
        sectionItems.push(newTask);
        
        // Update the map
        headingToItemsMap.set(targetHeading, sectionItems);
      } else {
        // No headings at all, just append to the end
        parsedMarkdown.push(newTask);
      }
      
      // Handle subtasks if any
      if (task.subTasks?.length) {
        for (const subTask of task.subTasks) {
          parsedMarkdown.push({
            type: "task",
            complete: false,
            name: subTask.text,
            details: null,
            prefix: "  - " // Indented subtask
          });
        }
      }
    }
    
    // Rebuild markdown with proper section organization
    const reorganizedMarkdown = rebuildMarkdownFromMap(headingToItemsMap, existingHeadings);
    
    // Update the project file with new markdown structure
    projectFile.markdown = reorganizedMarkdown;
    
    // Return updated content
    return update(originalContent, projectFile);
  }
  
  // Helper functions for task insertion
  function extractHeadings(markdown: ProjectMarkdown[]): string[] {
    return markdown
      .filter((item): item is typeof item & { type: "heading" } => item.type === "heading")
      .map(item => item.text);
  }
  
  function groupByHeading(markdown: ProjectMarkdown[]): Map<string, ProjectMarkdown[]> {
    const map = new Map<string, ProjectMarkdown[]>();
    let currentHeading: string | null = null;
    
    for (const item of markdown) {
      if (item.type === "heading") {
        currentHeading = item.text;
        if (!map.has(currentHeading)) {
          map.set(currentHeading, []);
        }
      } else if (currentHeading !== null) {
        const items = map.get(currentHeading) || [];
        items.push(item);
        map.set(currentHeading, items);
      }
    }
    
    return map;
  }
  
  function findBestHeadingMatch(
    suggestedHeading: string | undefined, 
    existingHeadings: string[]
  ): string | null {
    if (!suggestedHeading || existingHeadings.length === 0) return null;
    
    // Simple exact match first
    const exactMatch = existingHeadings.find(h => 
      h.toLowerCase() === suggestedHeading.toLowerCase());
    if (exactMatch) return exactMatch;
    
    // Simple word overlap scoring
    const words = suggestedHeading.toLowerCase().split(/\s+/);
    
    let bestMatch = null;
    let highestScore = 0;
    
    for (const heading of existingHeadings) {
      const headingWords = heading.toLowerCase().split(/\s+/);
      let score = 0;
      
      // Count matching words
      for (const word of words) {
        if (headingWords.includes(word)) score++;
      }
      
      if (score > highestScore) {
        highestScore = score;
        bestMatch = heading;
      }
    }
    
    // Return best match if score is high enough
    return highestScore > 0 ? bestMatch : null;
  }
  
  function rebuildMarkdownFromMap(
    headingMap: Map<string, ProjectMarkdown[]>,
    headingOrder: string[]
  ): ProjectMarkdown[] {
    const result: ProjectMarkdown[] = [];
    
    // Process headings in original order
    for (const heading of headingOrder) {
      // Add heading
      result.push({
        type: "heading",
        level: 1,
        text: heading
      });
      
      // Add items under this heading
      const items = headingMap.get(heading) || [];
      result.push(...items);
      
      // Add blank line if needed
      if (items.length > 0) {
        result.push({ type: "unrecognized", markdown: "" });
      }
    }
    
    return result;
  }
}
```

### Task Input Component

```typescript
// src/components/TaskInput.tsx
import { useAtom } from "jotai";
import { useState } from "react";
import { AITaskGenerator, type AIGeneratedTask } from "../lib/AITaskGenerator";
import { ProjectStateEditor } from "../lib/ProjectStateEditor";
import type { ProjectManager } from "../lib/project";

interface TaskInputProps {
  projectManager: ProjectManager;
  apiKey: string;
}

export function TaskInput({ projectManager, apiKey }: TaskInputProps) {
  const [description, setDescription] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  const handleGenerateTasks = async () => {
    if (!description.trim()) return;
    
    setIsLoading(true);
    setError(null);
    
    try {
      // Get existing headings from the current project
      const project = projectManager.getCurrentProject();
      if (!project) {
        setError("No project loaded");
        return;
      }
      
      const existingHeadings = project.projectFile.markdown
        .filter((item) => item.type === "heading")
        .map((item) => (item as any).text);
      
      // Generate tasks using AI
      const generator = new AITaskGenerator(apiKey);
      const tasks = await generator.generateTasks(description, existingHeadings);
      
      // Update the project with new tasks
      await projectManager.updateProject((projectFile) => {
        const updatedContent = ProjectStateEditor.insertAIGeneratedTasks(
          project.textContent,
          tasks
        );
        
        // Parse the updated content to get the new project state
        const updated = ProjectStateEditor.parse(updatedContent);
        Object.assign(projectFile, updated);
      });
      
      // Clear input after successful generation
      setDescription("");
    } catch (err) {
      setError(`Failed to generate tasks: ${err}`);
    } finally {
      setIsLoading(false);
    }
  };
  
  return (
    <div className="flex flex-col space-y-2">
      <textarea
        className="w-full p-2 border rounded resize-none focus:ring-2 focus:ring-blue-500"
        rows={3}
        placeholder="Describe your tasks in natural language..."
        value={description}
        onChange={(e) => setDescription(e.target.value)}
        disabled={isLoading}
      />
      
      <div className="flex justify-between items-center">
        <div className="text-sm text-red-500">{error}</div>
        <button
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:bg-blue-300"
          onClick={handleGenerateTasks}
          disabled={isLoading || !description.trim()}
        >
          {isLoading ? "Generating..." : "Generate Tasks"}
        </button>
      </div>
    </div>
  );
}
```

### ProjectManager Integration

Add a dedicated method to handle AI-generated tasks in ProjectManager:

```typescript
// src/lib/project.ts (addition)
import { type AIGeneratedTask } from "./AITaskGenerator";
import { ProjectStateEditor } from "./ProjectStateEditor";

export class ProjectManager {
  // ... existing code ...
  
  /**
   * Add AI-generated tasks to the current project
   */
  async addAIGeneratedTasks(tasks: AIGeneratedTask[]): Promise<void> {
    if (!this.currentFile) return;
    
    // Use ProjectStateEditor to generate updated content
    const updatedContent = ProjectStateEditor.insertAIGeneratedTasks(
      this.currentFile.textContent, 
      tasks
    );
    
    // Write the updated content to the file
    await writeTextFile(this.currentFile.fullPath, updatedContent);
    
    // Reload the project to reflect changes
    // Note: This isn't strictly necessary as the file watcher will trigger a reload,
    // but it provides immediate feedback
    const projectFile = ProjectStateEditor.parse(updatedContent);
    this.currentFile = {
      ...this.currentFile,
      textContent: updatedContent,
      projectFile,
    };
    
    await this.notifySubscribers(this.currentFile);
  }
}
```

## Tauri Backend Integration

To securely handle API keys and network requests, implement a Tauri command:

```rust
// src-tauri/src/commands.rs
use serde::{Deserialize, Serialize};
use tauri::command;
use reqwest::Client;

#[derive(Debug, Serialize, Deserialize)]
pub struct AIGeneratedTask {
    text: String,
    heading: Option<String>,
    priority: Option<u8>,
    #[serde(default)]
    sub_tasks: Vec<AIGeneratedTask>,
}

#[command]
pub async fn generate_ai_tasks(
    description: String, 
    system_prompt: String,
    api_key: String
) -> Result<Vec<AIGeneratedTask>, String> {
    let client = Client::new();
    
    // Example using OpenAI API
    let response = client.post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "gpt-4",
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": description}
            ],
            "temperature": 0.7,
            "response_format": { "type": "json_object" }
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let resp_json = response.json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    
    // Parse the response
    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("Invalid response format")?;
    
    // Parse JSON content into our task structure
    let tasks: Vec<AIGeneratedTask> = serde_json::from_str(content)
        .map_err(|e| format!("Failed to parse AI response: {}", e))?;
    
    Ok(tasks)
}
```

## Integration with App

Add the TaskInput component to the Planner view:

```typescript
// src/components/Planner.tsx
import { TaskInput } from "./TaskInput";
import { TaskList } from "./TaskList";
// ...other imports

export function Planner({ projectManager, apiKey }: PlannerProps) {
  // ...existing code
  
  return (
    <div className="flex flex-col gap-4 p-4">
      <h1 className="text-xl font-bold">Task Planner</h1>
      
      {/* AI Task Generation */}
      <div className="mb-4 bg-white p-4 rounded-lg shadow">
        <h2 className="text-lg font-semibold mb-2">Generate Tasks with AI</h2>
        <TaskInput 
          projectManager={projectManager} 
          apiKey={apiKey} 
        />
      </div>
      
      {/* Existing Task List */}
      <TaskList 
        tasks={tasks} 
        onCompleteTask={handleCompleteTask} 
      />
    </div>
  );
}
```

## Jotai Integration (Future)

When migrating to Jotai atoms as per the refactor plan:

```typescript
// src/atoms/ai-tasks.ts
import { atom } from "jotai";
import type { AIGeneratedTask } from "../lib/AITaskGenerator";
import { projectController } from "./project";

export const aiTasksController = (store: JotaiStore) => {
  // Atom for task generation status
  const isGeneratingAtom = atom<boolean>(false);
  
  // Atom for task description input
  const taskDescriptionAtom = atom<string>("");
  
  // Write-only atom for generating tasks
  const generateTasksAtom = atom(
    null,
    async (_get, set) => {
      const description = _get(taskDescriptionAtom);
      if (!description.trim()) return;
      
      set(isGeneratingAtom, true);
      
      try {
        // Generate tasks with AI
        const tasks = await aiTaskGenerator.generateTasks(
          description,
          _get(projectController.headingsAtom)
        );
        
        // Update project content
        set(projectController.updateContentAtom, (content) => {
          // Integration with ProjectStateEditor.insertAIGeneratedTasks
          // ...
          return true;
        });
        
        // Clear input
        set(taskDescriptionAtom, "");
      } catch (error) {
        console.error("Task generation failed:", error);
      } finally {
        set(isGeneratingAtom, false);
      }
    }
  );
  
  return {
    isGeneratingAtom,
    taskDescriptionAtom,
    generateTasks: () => store.set(generateTasksAtom)
  };
};
```

## Testing Strategy

1. **Unit Tests**:
   - Test task parsing and generation logic
   - Verify heading matching algorithm
   - Mock AI responses for consistent testing

2. **Integration Tests**:
   - Verify task insertion with various project structures
   - Test edge cases (empty projects, malformed markdown)

3. **E2E Tests**:
   - Test complete flow from user input to file update
   - Verify UI feedback during generation process

## Considerations

1. **Error Handling**:
   - Gracefully handle API failures
   - Provide helpful error messages to users
   - Implement retry logic for transient errors

2. **Performance**:
   - Debounce rapid task generation requests
   - Consider caching for large projects
   - Optimize markdown parsing for large files

3. **UX**:
   - Show loading states during generation
   - Provide feedback on task insertion
   - Consider progressive enhancement (work without AI)

4. **Security**:
   - Never expose API keys in the frontend
   - Use Tauri commands for API calls
   - Implement rate limiting

## Implementation Workflow

1. Start with the backend Tauri command
2. Implement the AITaskGenerator service
3. Add the task insertion logic to ProjectStateEditor
4. Create the TaskInput component
5. Integrate with ProjectManager
6. Add error handling and loading states
7. Test with various project structures

By following this guide, junior engineers should have a clear understanding of how to implement the AI task generation feature while maintaining consistency with the codebase's architecture and patterns. 