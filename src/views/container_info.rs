use ratatui::{buffer::Buffer, layout::Rect, style::{Color, Style, Styled, Stylize}, text::{Line, Span, Text}, widgets::{Block, Paragraph, ScrollbarState, Widget, Wrap}, Frame};

pub struct ContainerInfoData {
    pub id: String,
    pub name: String,
    pub image: String,
    pub created: String,
    pub state: String,
    pub mounts: String,
}

pub struct ContainerInfo {
    data: ContainerInfoData,
}

impl ContainerInfo {
    
    pub fn new(data: ContainerInfoData) -> Self {
        Self {
            data,        
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let title = self.data.name.clone()
            .light_blue()
            .bold();

        let block = Block::bordered()
            .gray()
            .title(title);

        let content = Paragraph::new(self.get_lines())
            .block(block)
            .left_aligned()
            .wrap(Wrap { trim: true });

        frame.render_widget(content, area);
    }

    fn get_lines(&self) -> Vec<Line> {
        let key_style = Style::new().light_blue().bold();

        let mut lines = [
            ("ID: ", self.data.id.clone()),
            ("Image: ", self.data.image.clone()),
            ("Created: ", self.data.created.clone()),
            ("State: ", self.data.state.clone()),
            //("Mounts: ", self.data.mounts.clone())

        ].into_iter().map(|(key, content)| {
            Line::from_iter([
                key.set_style(key_style),
                content.into()
            ])
        })
        .collect::<Vec<Line>>();

        lines.push(Line::from(""));
        lines.push(Line::from("Mounts: ".set_style(key_style)));

        let mut mounts_line = self.data.mounts
            .split("\n")
            .map(Line::raw)
            .collect::<Vec<Line>>();
        
        lines.append(&mut mounts_line);

        lines
    }
}
