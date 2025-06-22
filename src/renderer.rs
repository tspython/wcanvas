use crate::app_state::State;

impl State {
    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.gpu.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.gpu.render_pipeline);
            render_pass.set_bind_group(0, &self.canvas.uniform_bind_group, &[]);

            if let (Some(vertex_buffer), Some(index_buffer)) =
                (&self.geometry.vertex, &self.geometry.index)
            {
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.geometry.count, 0, 0..1);
            }

            render_pass.set_pipeline(&self.gpu.ui_render_pipeline);
            render_pass.set_bind_group(0, &self.ui_screen.bind_group, &[]);

            if let (Some(ui_vertex_buffer), Some(ui_index_buffer)) =
                (&self.ui_geo.vertex, &self.ui_geo.index)
            {
                render_pass.set_vertex_buffer(0, ui_vertex_buffer.slice(..));
                render_pass.set_index_buffer(ui_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.ui_geo.count, 0, 0..1);
            }
        }

        self.text_renderer.draw(&mut encoder, &view, &self.canvas.uniform_bind_group);
        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
