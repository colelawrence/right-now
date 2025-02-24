# Creating Custom Soundbites for Right Now

Right Now uses brief audio clips (“soundbites”) to keep you motivated and alert during your work sessions. By default, it includes a standard set of soundbites, but you can easily add your own to personalize your experience. This guide explains the process of creating your own soundpack and provides tips for making it sound great.

## Overview of Sound Categories

Right Now defines several different categories of soundbites, each corresponding to specific events in the Pomodoro workflow. In the code snippet, this is represented as a \(SoundPackPhrases\) type with the following keys:

1. **session_start**  
   - Played when you begin a new Pomodoro session.  
   - Ideal for a short burst of motivation or a friendly greeting.

2. **session_ended**  
   - Played when the session is over.  
   - Encouraging phrases to congratulate the user on finishing.

3. **todo_complete**  
   - Played when a user completes a task.  
   - Perfect for celebratory or congratulatory clips.

4. **break_approaching**  
   - Played shortly before a break is about to occur.  
   - Helps the user wind down and wrap up a current task.

5. **break_start**  
   - Played when a break actually begins.  
   - Ideal for short prompts that suggest relaxation.

6. **break_end_approaching**  
   - Played shortly before a break is over.  
   - Encourages users to prepare to resume work.

7. **work_resumed**  
   - Played when work resumes after a break.  
   - Motivational phrases to regain focus.

8. **break_nudge**  
   - Played when the system detects you have worked for a while and it's time for a break.  
   - Gentle reminders to stand up, stretch, or rest your eyes.

Each category can hold multiple soundbites. If you add multiple sound clips for a single category, Right Now will choose from them at random to keep the experience fresh.

---

## How to Record

### Recommended Equipment
- A basic USB microphone or your smartphone's voice recorder can work well.
- For higher quality, consider using a microphone that supports at least 44.1 kHz sample rate.

### Recording Environment
- Try to record in a quiet space to avoid background noise.
- If possible, place soft materials (e.g., blankets, pillows) around you to dampen echoes.

### Recording Settings
- Format: MP3 or WAV (at least 44.1 kHz, 16-bit).  
- Duration: Aim for **2–5 seconds** for each clip. If your style needs longer, 10 seconds is usually the upper limit.  
- Volume: Keep your volume levels consistent across all clips.

### Content Suggestions

1. **session_start**  
   - Tone: Upbeat, energizing.  
   - Example lines:  
     - "Alright, let's get this done!"  
     - "It's go time—focus on what matters!"

2. **session_ended**  
   - Tone: Positive, congratulatory.  
   - Example lines:  
     - "Great job, session complete!"  
     - "Session ended, well done!"

3. **todo_complete**  
   - Tone: Celebratory, encouraging.  
   - Example lines:  
     - "Task finished—way to go!"  
     - "You did it! Another to-do is off the list."

4. **break_approaching**  
   - Tone: Soothing, gentle reminder.  
   - Example lines:  
     - "Break's almost here. Time to wrap up."  
     - "Prepare to rest—your break is coming."

5. **break_start**  
   - Tone: Relaxing, calming.  
   - Example lines:  
     - "Time for a quick break—take it easy."  
     - "Break time! Breathe and recharge."

6. **break_end_approaching**  
   - Tone: Light motivational push.  
   - Example lines:  
     - "Break is wrapping up—almost time to work again."  
     - "Wrap up the break—work resumes soon."

7. **work_resumed**  
   - Tone: Encouraging, enthusiastic.  
   - Example lines:  
     - "Back to it—let's crush these tasks!"  
     - "Work resumed. You got this!"

8. **break_nudge**  
   - Tone: Gentle, friendly reminder to avoid burnout.  
   - Example lines:  
     - "You've been at it for a while—maybe it's time for a rest."  
     - "Time to step away—take a quick break to recharge."

---

## File Naming

It's best to use descriptive and consistent names.

1. **Simple naming example**:  
   - \(session_start_01.mp3\), \(session_start_02.mp3\), \(break_start_01.mp3\), etc.

2. **Wildcard (random selection)**:  
   - Right Now will automatically detect files prefixed with the category name (e.g., \(session_start.*.mp3\)). That means multiples like \(session_start.1.mp3\), \(session_start.2.mp3\) will be rotated automatically.

---

## Tips for Quality Soundbites

1. **Consistency**: Keep your microphone distance and tone even.  
2. **Clarity**: Speak clearly and avoid rushing.  
3. **Editing**: Consider trimming pauses or breath sounds for a cleaner clip.  
4. **Energy**: Vary the vibe for different categories. For instance, a calm or soothing tone for break notifications, and more energy for session starts.

---

## Implementation Steps

1. **Record your clips** using the guidelines above.  
2. **Name your files** according to the category (e.g., \(session_start.1.mp3\)).  
3. **Place them in your custom soundpack folder**. For example, \(assets/soundpacks/my-custom-pack\).  
4. **Update the Right Now configuration** to point to your custom soundpack (this may vary based on your setup).  
5. **Launch or reload Right Now** to ensure the new clips are recognized.  
6. **Verify** each category triggers your custom clip during the appropriate event.

By following these recommendations, you'll create a set of motivational and clear soundbites that help you (and others) have a more engaging, focused experience with Right Now. Happy recording! 