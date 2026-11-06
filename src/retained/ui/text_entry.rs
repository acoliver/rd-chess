use super::{GameUi, Screen};
use worldrunner::{
    simulation::{ComponentDesc, TextInput, WorldError},
    ui::{Clipboard, ClipboardCommand, UiControl, UiResponse, UiTextField},
};

impl GameUi {
    pub(super) fn label_page(&mut self) -> Result<(), WorldError> {
        let panel = self.native.panel([620., 230.])?;
        self.pages.push((Screen::SaveLabel, panel));
        self.native
            .text(panel, "Save label", [580., 36.], [0., 75.])?;
        let object = self.native.text(panel, "", [560., 42.], [0., 10.])?;
        self.native.control(object, UiControl::text_field());
        self.native
            .router
            .semantics
            .insert(object, worldrunner::ui::UiSemantics::label("Save label"));
        self.label_field = Some(object);
        self.native.text(
            panel,
            "Enter: save   Escape: cancel\n1-24 printable ASCII characters",
            [580., 58.],
            [0., -65.],
        )?;
        Ok(())
    }
    pub(in crate::retained) fn edit_label(&mut self, text: &str) -> Result<(), String> {
        let object = self.label_field.expect("label field");
        let ComponentDesc::Text(mut run) = self
            .native
            .world
            .component(self.native.text_handle(object))
            .map_err(|e| e.to_string())?
            .desc
            .clone()
        else {
            unreachable!("label text")
        };
        text.clone_into(&mut run.text);
        // Allow whitespace around a valid label and show validation errors on Enter.
        let field = UiTextField::new(run, 256, &self.native.fonts)
            .map_err(|e| format!("Label error: {e:?}"))?;
        self.native.router.text_fields.insert(object, field);
        self.dirty = true;
        Ok(())
    }
    pub(in crate::retained) fn label_value(&self) -> &str {
        self.native.router.text_fields[&self.label_field.expect("label field")].value()
    }
    pub(in crate::retained) fn text(
        &mut self,
        text: &TextInput,
        viewport: [u32; 2],
    ) -> Result<UiResponse, WorldError> {
        let layout = self
            .native
            .world
            .ui_layout_with(viewport.into(), &self.native.router.layout)?;
        self.dirty = true;
        self.native
            .router
            .text(&self.native.world, &layout, &self.native.fonts, text)
    }
    pub(in crate::retained) fn clipboard(
        &mut self,
        command: ClipboardCommand,
        clipboard: &mut dyn Clipboard,
        viewport: [u32; 2],
    ) -> Result<UiResponse, WorldError> {
        let layout = self
            .native
            .world
            .ui_layout_with(viewport.into(), &self.native.router.layout)?;
        self.dirty = true;
        self.native.router.clipboard(
            &self.native.world,
            &layout,
            &self.native.fonts,
            command,
            clipboard,
        )
    }
    pub(in crate::retained) fn text_input_area(
        &self,
        viewport: [u32; 2],
    ) -> Result<Option<[f64; 4]>, String> {
        if viewport.contains(&0) {
            return Ok(None);
        }
        let layout = self
            .native
            .world
            .ui_layout_with(viewport.into(), &self.native.router.layout)
            .map_err(|e| e.to_string())?;
        self.native
            .router
            .text_input_area(&self.native.world, &layout, &self.native.fonts)
            .map_err(|e| e.to_string())
    }
}
