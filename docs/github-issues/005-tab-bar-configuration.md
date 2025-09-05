# [ENHANCEMENT] Add Tab Bar Configuration Options

## Priority  
🟡 **Medium** - User experience improvement

## Description
The tab bar implementation has a TODO comment indicating missing configuration for the close button display. This should be part of a broader effort to make the tab bar more configurable.

## Current Status
Implemented in codebase (tab bar config and rendering updated). TODO comment removed and config respected for close button, modified indicator, numbering, new-tab button, and width constraints.

### Location with TODO
- **File**: `openagent-terminal/src/display/tab_bar.rs`
- Status: TODO removed; rendering now checks `workspace.tab_bar.show_close_button` and `close_button_on_hover`.

## Proposed Configuration Options

### Core Tab Bar Settings
```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TabBarConfig {
    /// Whether to show tab bar
    pub show: bool,
    
    /// Position of tab bar
    pub position: TabBarPosition,
    
    /// Visual style
    pub style: TabBarStyle,
    
    /// Behavior options
    pub behavior: TabBarBehavior,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TabBarBehavior {
    /// Show close button on tabs
    pub show_close_button: bool,
    
    /// Show close button only on hover
    pub close_button_on_hover: bool,
    
    /// Show modified indicator
    pub show_modified_indicator: bool,
    
    /// Maximum tab width in characters
    pub max_tab_width: Option<usize>,
    
    /// Minimum tab width in characters  
    pub min_tab_width: Option<usize>,
    
    /// Show tab numbers/indices
    pub show_tab_numbers: bool,
    
    /// Show new tab button
    pub show_new_tab_button: bool,
    
    /// Click behavior
    pub middle_click_action: MiddleClickAction,
    pub double_click_action: DoubleClickAction,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum MiddleClickAction {
    Close,
    NewTab,
    None,
}

#[derive(Debug, Clone, Deserialize, Serialize)]  
pub enum DoubleClickAction {
    Rename,
    Duplicate,
    None,
}
```

### Visual Style Options
```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TabBarStyle {
    /// Tab separator character
    pub separator: String,
    
    /// Close button character  
    pub close_button: String,
    
    /// Modified indicator character
    pub modified_indicator: String,
    
    /// New tab button text
    pub new_tab_button: String,
    
    /// Colors (if not using theme defaults)
    pub colors: Option<TabBarColors>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TabBarColors {
    pub active_fg: Option<Color>,
    pub active_bg: Option<Color>, 
    pub inactive_fg: Option<Color>,
    pub inactive_bg: Option<Color>,
    pub hover_fg: Option<Color>,
    pub hover_bg: Option<Color>,
    pub modified_fg: Option<Color>,
    pub border: Option<Color>,
}
```

## Implementation Plan

### Phase 1: Basic Configuration
1. **Add Configuration Structure**
   ```rust
   // In config/workspace.rs
   #[derive(Debug, Clone, Deserialize, Serialize)]
   pub struct TabBarConfig {
       #[serde(default = "default_show")]
       pub show: bool,
       
       #[serde(default)]
       pub position: TabBarPosition,
       
       #[serde(default)]
       pub show_close_button: bool,
       
       #[serde(default = "default_close_on_hover")]
       pub close_button_on_hover: bool,
       
       #[serde(default = "default_modified_indicator")]
       pub show_modified_indicator: bool,
       
       #[serde(default)]
       pub max_tab_width: Option<usize>,
   }
   
   fn default_show() -> bool { true }
   fn default_close_on_hover() -> bool { false }  
   fn default_modified_indicator() -> bool { true }
   ```

2. **Update Tab Bar Rendering**
   ```rust
   // In display/tab_bar.rs
   fn draw_tab_bar(&mut self, config: &UiConfig, tab_manager: &TabManager, position: TabBarPosition) -> Option<TabBarGeometry> {
       let tab_config = &config.workspace.tab_bar;
       
       // ... existing code ...
       
       // Draw close button based on configuration
       if tab_config.show_close_button && (!tab_config.close_button_on_hover || is_hover_tab) {
           let close_x = current_x + tab_width.saturating_sub(2);
           if close_x > current_x {
               let close_point = Point::new(start_line, Column(close_x));
               let close_color = if is_hover_close {
                   tokens.accent
               } else if is_active {
                   active_fg  
               } else {
                   tokens.text_muted
               };
               
               let close_char = &tab_config.style.close_button;
               self.draw_tab_text(close_point, close_color, bg, close_char, 1);
           }
       }
       
       // Show modified indicator based on configuration
       if tab.modified && tab_config.show_modified_indicator {
           let indicator = &tab_config.style.modified_indicator;
           tab_text.push_str(indicator);
           tab_text.push(' ');
       }
   }
   ```

### Phase 2: Advanced Features
1. **Tab Numbering**
   ```rust
   // Add tab numbers if configured
   if tab_config.show_tab_numbers {
       let tab_number = format!("{}:", index + 1);
       tab_text.insert_str(0, &tab_number);
   }
   ```

2. **Interactive Features**
   ```rust
   // Enhanced click handling
   pub fn handle_tab_bar_click(&self, tab_manager: &TabManager, position: TabBarPosition, mouse_x: usize, mouse_y: usize, button: MouseButton) -> Option<TabBarAction> {
       let tab_config = &self.config.workspace.tab_bar;
       
       match button {
           MouseButton::Left => {
               // Normal click handling
               self.handle_left_click(tab_manager, mouse_x, mouse_y)
           },
           MouseButton::Middle => {
               match tab_config.middle_click_action {
                   MiddleClickAction::Close => Some(TabBarAction::CloseTab(tab_id)),
                   MiddleClickAction::NewTab => Some(TabBarAction::NewTab),
                   MiddleClickAction::None => None,
               }
           },
           MouseButton::Right => {
               // Context menu
               Some(TabBarAction::ShowContextMenu(tab_id, mouse_x, mouse_y))
           },
       }
   }
   ```

3. **Tab Width Management**
   ```rust
   fn calculate_tab_width(&self, tab_manager: &TabManager, available_width: usize) -> usize {
       let tab_config = &self.config.workspace.tab_bar;
       let num_tabs = tab_manager.tab_count();
       
       if num_tabs == 0 {
           return 0;
       }
       
       let mut ideal_width = available_width / num_tabs;
       
       // Apply min/max constraints
       if let Some(min_width) = tab_config.min_tab_width {
           ideal_width = ideal_width.max(min_width);
       }
       
       if let Some(max_width) = tab_config.max_tab_width {
           ideal_width = ideal_width.min(max_width);
       }
       
       ideal_width
   }
   ```

### Phase 3: Theme Integration
1. **Theme-Aware Colors**
   ```rust
   fn get_tab_colors(&self, config: &UiConfig) -> TabBarColors {
       let theme_tokens = &config.resolved_theme.as_ref().unwrap().tokens;
       let tab_config = &config.workspace.tab_bar;
       
       TabBarColors {
           active_fg: tab_config.colors.as_ref()
               .and_then(|c| c.active_fg)
               .unwrap_or(theme_tokens.text),
           active_bg: tab_config.colors.as_ref()
               .and_then(|c| c.active_bg) 
               .unwrap_or(theme_tokens.surface),
           // ... other colors
       }
   }
   ```

## Configuration Examples

### Default Configuration
```yaml
workspace:
  tab_bar:
    show: true
    position: top
    show_close_button: true
    close_button_on_hover: false
    show_modified_indicator: true
    show_tab_numbers: false
    show_new_tab_button: true
    max_tab_width: 25
    min_tab_width: 8
    
    style:
      separator: "│"
      close_button: "×"
      modified_indicator: "●"
      new_tab_button: "[+]"
      
    middle_click_action: close
    double_click_action: rename
```

### Minimal Configuration
```yaml
workspace:
  tab_bar:
    show: true
    show_close_button: false
    show_modified_indicator: false
    style:
      separator: " "
      new_tab_button: "+"
```

### Power User Configuration
```yaml
workspace:
  tab_bar:
    show: true
    position: bottom
    show_close_button: true
    close_button_on_hover: true
    show_modified_indicator: true
    show_tab_numbers: true
    max_tab_width: 30
    min_tab_width: 10
    
    style:
      separator: " ┃ "
      close_button: "⊗"
      modified_indicator: "◉"
      new_tab_button: "⊕"
      
    colors:
      active_fg: "#ffffff"
      active_bg: "#404040"
      hover_fg: "#00ff00"
      
    middle_click_action: close
    double_click_action: duplicate
```

## Files to Modify
- `openagent-terminal/src/config/workspace.rs`
- `openagent-terminal/src/display/tab_bar.rs`
- `openagent-terminal/src/display/mod.rs` 
- Configuration schema documentation

## Testing Requirements
- [ ] All configuration options work as expected
- [ ] Tab bar responds correctly to different screen sizes
- [ ] Color theming integrates properly
- [ ] Click handling works for all configured actions
- [ ] Configuration validation prevents invalid settings

## Labels
- `priority/medium`
- `type/enhancement`
- `component/ui`
- `component/tabs`

## Definition of Done
- [x] TODO comment resolved
- [x] Configuration structure implemented (close_button_on_hover, show_new_tab_button, show_tab_numbers, min/max tab widths)
- [x] Tab bar config options functional in rendering and click/drag
- [ ] Theme integration working (advanced style/color overrides pending)
- [ ] Click handling enhanced (middle/double click actions pending)
- [ ] Tests passing (integration tests to be expanded)
- [x] Documentation updated with configuration examples
- [x] Default configuration provides good UX
