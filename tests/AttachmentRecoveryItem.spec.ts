import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import AttachmentRecoveryItem from '../src/components/AttachmentRecoveryItem.vue'

const image = {
  attachmentId: '018f8f2a-7c20-7abc-8def-1234567890ad',
  fileName: 'diagram.png',
  mediaType: 'image/png',
  byteSize: 4096,
  caption: 'Geometry diagram'
}

describe('AttachmentRecoveryItem', () => {
  it('preserves metadata and offers an idempotent retry for a failed transfer', async () => {
    const wrapper = mount(AttachmentRecoveryItem, {
      props: { image, status: 'failed', errorCode: 'SYNC_ATTACHMENT_HASH_MISMATCH' }
    })
    expect(wrapper.text()).toContain('diagram.png')
    expect(wrapper.text()).toContain('The question content remains available.')
    expect(wrapper.text()).toContain('SYNC_ATTACHMENT_HASH_MISMATCH')
    await wrapper.get('button').trigger('click')
    expect(wrapper.emitted('retry')).toHaveLength(1)
  })

  it('does not replace available local bytes with a placeholder', () => {
    const wrapper = mount(AttachmentRecoveryItem, { props: { image, status: 'synced' } })
    expect(wrapper.find('[role="status"]').exists()).toBe(false)
    expect(wrapper.text()).toContain('Available locally')
  })

  it('keeps pending metadata visible without an unsafe manual restart', () => {
    const wrapper = mount(AttachmentRecoveryItem, { props: { image, status: 'pending' } })
    expect(wrapper.text()).toContain('Metadata is safe')
    expect(wrapper.find('button').exists()).toBe(false)
  })
})
