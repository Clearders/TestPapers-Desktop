import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import AppIcon from '../src/components/AppIcon.vue'

describe('AppIcon provenance port', () => {
  it('preserves the Web sparkles geometry and remains decorative', () => {
    const wrapper = mount(AppIcon, { props: { name: 'sparkles' } })
    expect(wrapper.get('svg').attributes('aria-hidden')).toBe('true')
    expect(wrapper.findAll('path')).toHaveLength(3)
    expect(wrapper.find('path').attributes('d')).toBe('M12 3l1.7 4.3L18 9l-4.3 1.7L12 15l-1.7-4.3L6 9l4.3-1.7L12 3Z')
  })
})
